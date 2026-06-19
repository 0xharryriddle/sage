//! Runnable local SAGE testnet harness.
//!
//! Spins up an N-validator `LocalRuntime` with a configurable migration
//! strategy and migration schedule, runs it to a target height (or timeout),
//! and prints the resulting `NodeMetrics` as JSON. This is the runnable entry
//! point behind the multi-validator testbed: a seed-sweep / CI driver can call
//! it repeatedly with different `--strategy`/`--seed` and parse the JSON line.
//!
//! It intentionally has no external CLI dependency — a tiny `--key value`
//! parser keeps the crate's dependency surface unchanged.
//!
//! Example:
//!   run_node --n 6 --f 1 --strategy hardfork --max-height 8 --json

use sage_controller::Schedule;
use sage_core::Height;
use sage_node::config::{NodeConfig, NodeStrategy};
use sage_node::LocalRuntime;
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }
    let opts = parse_opts(&args);

    let n: u32 = opt_u64(&opts, "n", 6) as u32;
    let f: u32 = opt_u64(&opts, "f", 1) as u32;
    let max_height = Height::new(opt_u64(&opts, "max-height", 8));
    let h_d = opt_u64(&opts, "h-d", 2);
    let h_c = opt_u64(&opts, "h-c", 4);
    let h_r = opt_u64(&opts, "h-r", 8);
    let kappa = opt_u64(&opts, "kappa", 1);
    let tau = opt_u64(&opts, "tau", 1);
    let max_secs = opt_u64(&opts, "max-secs", 10);
    let seed = opt_u64(&opts, "seed", 42);
    let json = opts.contains_key("json");

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

    let mut cfg = NodeConfig::local_testnet(n, f, schedule, max_height).with_strategy(strategy);
    cfg.max_runtime_secs = max_secs;
    cfg.workload_seed = seed;

    let mut runtime = match LocalRuntime::new(cfg) {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to build runtime: {e:?}");
            std::process::exit(1);
        }
    };

    match runtime.run() {
        Ok(metrics) => {
            if json {
                match serde_json::to_string(&metrics) {
                    Ok(s) => println!("{s}"),
                    Err(e) => {
                        eprintln!("serialize error: {e}");
                        std::process::exit(1);
                    }
                }
            } else {
                println!("strategy={strategy:?} n={n} f={f}");
                println!("  finalized_blocks    = {}", metrics.finalized_blocks);
                println!("  max_finalized_height= {}", metrics.max_finalized_height);
                println!("  migration_success   = {}", metrics.migration_success);
                println!("  safety_violation    = {}", metrics.safety_violation);
                println!("  duration_secs       = {:.3}", metrics.total_duration_secs);
            }
        }
        Err(e) => {
            eprintln!("run error: {e:?}");
            std::process::exit(1);
        }
    }
}

/// Parse `--key value` and bare `--flag` pairs into a map.
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

fn print_help() {
    println!(
        "run_node — local SAGE testnet harness\n\n\
         Options (all optional, shown with defaults):\n\
         \x20 --n 6                number of validators\n\
         \x20 --f 1                faults tolerated (BFT quorum = 2f+1)\n\
         \x20 --strategy sage      sage|hardfork|sageblind|stw|reconfig\n\
         \x20 --max-height 8       target height to finalize\n\
         \x20 --h-d 2 --h-c 4 --h-r 8   migration schedule heights\n\
         \x20 --kappa 1 --tau 1    stability threshold / tau blocks\n\
         \x20 --max-secs 10        wall-clock safety valve\n\
         \x20 --seed 42            workload seed (vary for seed sweeps)\n\
         \x20 --json               emit metrics as a single JSON line\n"
    );
}
