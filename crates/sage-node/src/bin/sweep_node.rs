//! Real-node seed/validator sweep harness (testbed milestone M4).
//!
//! Drives the wall-clock `LocalRuntime` (NOT the deterministic `sage-sim`)
//! across a grid of validator counts and seeds, and emits one CSV row per
//! trial. This is only trustworthy because the PoA->HotStuff cutover stall is
//! resolved (see `docs/engineering/PACEMAKER_FIX_PLAN.md`): both the view-vs-height leader
//! bug (Bug A) and the dead monotonic timer (Bug C) are fixed, so n=4..40 cross
//! cutover reliably across seeds. Before that fix this sweep would have
//! manufactured unreliable numbers, which is why it was deliberately deferred.
//!
//! Like `run_node`, it uses a tiny `--key value` parser and no external CLI
//! dependency, keeping the crate's dependency surface unchanged.
//!
//! Example:
//!   sweep_node --validators 4,7,10,16,20 --seeds 0,1,2,3,4 \
//!     --strategy sage --max-height 8 --out results/raw/node_sweep.csv

use sage_controller::Schedule;
use sage_core::Height;
use sage_node::config::{NodeConfig, NodeStrategy};
use sage_node::{LocalRuntime, NodeMetrics};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }
    let opts = parse_opts(&args);

    let validators = opt_u32_list(&opts, "validators", &[4, 7, 10, 13, 16, 20]);
    let seeds = opt_u64_list(&opts, "seeds", &[0, 1, 2, 3, 4]);
    let f = opt_u64(&opts, "f", 1) as u32;
    let max_height = Height::new(opt_u64(&opts, "max-height", 8));
    let h_d = opt_u64(&opts, "h-d", 2);
    let h_c = opt_u64(&opts, "h-c", 4);
    let h_r = opt_u64(&opts, "h-r", 8);
    let kappa = opt_u64(&opts, "kappa", 1);
    let tau = opt_u64(&opts, "tau", 1);
    let max_secs = opt_u64(&opts, "max-secs", 10);
    let out = opts
        .get("out")
        .cloned()
        .unwrap_or_else(|| "results/raw/node_sweep.csv".to_string());

    let strategy = match opts.get("strategy").map(|s| s.as_str()) {
        None | Some("sage") => NodeStrategy::Sage,
        Some("hardfork") => NodeStrategy::HardFork,
        Some("sageblind") | Some("blind") => NodeStrategy::SageBlind,
        Some("stoptheworld") | Some("stw") => NodeStrategy::StopTheWorld,
        Some("reconfig") | Some("reconfigonly") => NodeStrategy::ReconfigOnly,
        Some("coxstyle") | Some("cox") => NodeStrategy::CoxStyle,
        Some(other) => {
            eprintln!("unknown strategy '{other}', valid: sage|hardfork|sageblind|stw|reconfig");
            std::process::exit(2);
        }
    };

    let schedule = Schedule {
        h_d: Height::new(h_d),
        h_c: Height::new(h_c),
        h_r: Height::new(h_r),
        kappa,
        tau_blocks: tau,
    };

    let mut rows: Vec<Row> = Vec::new();
    let mut successes = 0usize;
    let mut total = 0usize;

    for &n in &validators {
        for &seed in &seeds {
            total += 1;
            let mut cfg =
                NodeConfig::local_testnet(n, f, schedule, max_height).with_strategy(strategy);
            cfg.max_runtime_secs = max_secs;
            cfg.workload_seed = seed;

            let (status, m) = match LocalRuntime::new(cfg) {
                Ok(mut rt) => match rt.run() {
                    Ok(m) => {
                        if m.migration_success {
                            successes += 1;
                        }
                        ("ok".to_string(), m)
                    }
                    Err(e) => (format!("run_error:{e:?}"), NodeMetrics::default()),
                },
                Err(e) => (format!("create_error:{e:?}"), NodeMetrics::default()),
            };

            eprintln!(
                "n={n} seed={seed} success={} h={} secs={:.3} [{status}]",
                m.migration_success, m.max_finalized_height, m.total_duration_secs
            );

            rows.push(Row {
                strategy: format!("{strategy:?}"),
                n,
                f,
                seed,
                h_c,
                finalized_blocks: m.finalized_blocks,
                max_finalized_height: m.max_finalized_height,
                migration_success: m.migration_success,
                safety_violation: m.safety_violation,
                total_duration_secs: m.total_duration_secs,
                run_status: status,
            });
        }
    }

    if let Err(e) = write_csv(&out, &rows) {
        eprintln!("failed to write {out}: {e}");
        std::process::exit(1);
    }

    println!(
        "wrote {} rows to {out} ({}/{} migration_success)",
        rows.len(),
        successes,
        total
    );
}

struct Row {
    strategy: String,
    n: u32,
    f: u32,
    seed: u64,
    h_c: u64,
    finalized_blocks: u64,
    max_finalized_height: u64,
    migration_success: bool,
    safety_violation: bool,
    total_duration_secs: f64,
    run_status: String,
}

const CSV_HEADER: &str = "strategy,n,f,seed,h_c,finalized_blocks,max_finalized_height,migration_success,safety_violation,total_duration_secs,run_status";

fn write_csv(path: impl AsRef<Path>, rows: &[Row]) -> std::io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut out = String::with_capacity(rows.len() * 64 + CSV_HEADER.len() + 1);
    out.push_str(CSV_HEADER);
    out.push('\n');
    for r in rows {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{:.6},{}\n",
            r.strategy,
            r.n,
            r.f,
            r.seed,
            r.h_c,
            r.finalized_blocks,
            r.max_finalized_height,
            r.migration_success,
            r.safety_violation,
            r.total_duration_secs,
            r.run_status,
        ));
    }
    fs::write(path, out)
}

fn parse_opts(args: &[String]) -> BTreeMap<String, String> {
    let mut opts = BTreeMap::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if let Some(key) = arg.strip_prefix("--") {
            if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                opts.insert(key.to_string(), args[i + 1].clone());
                i += 2;
            } else {
                opts.insert(key.to_string(), String::new());
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    opts
}

fn opt_u64(opts: &BTreeMap<String, String>, key: &str, default: u64) -> u64 {
    opts.get(key)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

fn opt_u32_list(opts: &BTreeMap<String, String>, key: &str, default: &[u32]) -> Vec<u32> {
    match opts.get(key) {
        Some(v) if !v.is_empty() => v
            .split(',')
            .filter_map(|s| s.trim().parse::<u32>().ok())
            .collect(),
        _ => default.to_vec(),
    }
}

fn opt_u64_list(opts: &BTreeMap<String, String>, key: &str, default: &[u64]) -> Vec<u64> {
    match opts.get(key) {
        Some(v) if !v.is_empty() => v
            .split(',')
            .filter_map(|s| s.trim().parse::<u64>().ok())
            .collect(),
        _ => default.to_vec(),
    }
}

fn print_help() {
    println!(
        "sweep_node — real-node seed/validator sweep (testbed M4)\n\n\
         Drives the wall-clock LocalRuntime across a grid of validator counts\n\
         and seeds, emitting one CSV row per trial.\n\n\
         Options (all optional, shown with defaults):\n\
         \x20 --validators 4,7,10,13,16,20   comma-separated validator counts\n\
         \x20 --seeds 0,1,2,3,4              comma-separated workload seeds\n\
         \x20 --f 1                          faults tolerated (BFT quorum = 2f+1)\n\
         \x20 --strategy sage                sage|hardfork|sageblind|stw|reconfig\n\
         \x20 --max-height 8                 target height to finalize\n\
         \x20 --h-d 2 --h-c 4 --h-r 8        migration schedule heights\n\
         \x20 --kappa 1 --tau 1              stability threshold / tau blocks\n\
         \x20 --max-secs 10                  per-trial wall-clock safety valve\n\
         \x20 --out results/raw/node_sweep.csv   output CSV path\n"
    );
}
