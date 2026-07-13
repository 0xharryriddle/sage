//! Dual-run overhead microbenchmark.
//!
//! This is a local simulator-capacity benchmark: it runs the same workload through
//! a baseline PoA/reconfiguration path and through SAGE dual-run shadow validation,
//! then records committed transaction throughput and process CPU cost.
use clap::Parser;
use sage_experiments::common::{load_config, write_csv};
use sage_experiments::error::ExperimentResult;
use sage_sim::{SimConfig, Simulation, StrategyKind};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "config/rq1.toml")]
    config: PathBuf,
    #[arg(long, default_value = "results/raw")]
    out_dir: PathBuf,
    #[arg(long, value_delimiter = ',', default_value = "10,25,50,100,250,500")]
    txs_per_block: Vec<usize>,
    #[arg(long, value_delimiter = ',')]
    seeds: Option<Vec<u64>>,
    #[arg(long, default_value = "5")]
    trials: u64,
    /// Optional MODELED cross-region WAN sweep: comma-separated mean one-way
    /// message delays in MICROSECONDS, applied to the deterministic simulator's
    /// `mean_delay_micros`. When set, emits a separate `wan_throughput.csv` with
    /// a 3-arm (baseline/SAGE/Cox-style) committed-TPS comparison at each delay,
    /// under a fixed txs/block. This is a MODELED cross-region latency in the
    /// controlled simulator (the fair environment), NOT a physically
    /// geo-distributed real-host deployment. Example continental/intercontinental
    /// one-way delays: 50000,80000,120000 (µs) i.e. 50/80/120 ms.
    #[arg(long, value_delimiter = ',')]
    wan_delays_micros: Option<Vec<u64>>,
    /// Fixed txs/block used for the WAN sweep (kept constant so the only variable
    /// is the modeled inter-region delay). Ignored unless `--wan-delays-micros`.
    #[arg(long, default_value = "200")]
    wan_txs_per_block: usize,
}

#[derive(Debug, Clone, Serialize)]
struct OverheadRow {
    experiment: String,
    mode: String,
    strategy: String,
    seed: u64,
    n: u32,
    f: u64,
    txs_per_block: usize,
    offered_tps_model: u64,
    committed_tps_wall: f64,
    wall_seconds: f64,
    cpu_user_pct: f64,
    cpu_system_pct: f64,
    cpu_total_pct: f64,
    rss_mb_end: f64,
    finalized_blocks: u64,
    max_finalized_height: u64,
    committed_txs: u64,
    shadow_validation_count_model: u64,
    shadow_validation_ns_total_proxy: u128,
    shadow_validation_ns_per_tx_proxy: f64,
    cutover_height: Option<u64>,
    migration_success: bool,
    safety_violation: bool,
    generator_saturated: bool,
    run_status: String,
}

#[derive(Debug, Clone, Copy)]
struct ProcSample {
    user_ticks: u64,
    system_ticks: u64,
    rss_pages: i64,
}

fn main() -> ExperimentResult<()> {
    let cli = Cli::parse();
    fs::create_dir_all(&cli.out_dir)?;
    let base_cfg = load_config(&cli.config)?;
    let seeds = cli.seeds.unwrap_or_else(|| (0..cli.trials).collect());
    let ticks_per_second = clock_ticks_per_second();
    let page_size = page_size_bytes();
    let mut rows = Vec::new();

    for &txs_per_block in &cli.txs_per_block {
        for &seed in &seeds {
            rows.push(run_case(
                &base_cfg,
                seed,
                txs_per_block,
                "baseline_poa",
                StrategyKind::ReconfigOnly,
                ticks_per_second,
                page_size,
            ));
            rows.push(run_case(
                &base_cfg,
                seed,
                txs_per_block,
                "sage_dual_run",
                StrategyKind::Sage,
                ticks_per_second,
                page_size,
            ));
            // Cox-style live homogeneous switch: the closest live-switch SOTA
            // baseline, run under IDENTICAL config (same seed, txs_per_block, n,
            // f, load, window) so the throughput comparison is a fair
            // same-conditions measurement ("both run 100m"). See
            // docs/EXPERIMENT_FAIRNESS.md.
            rows.push(run_case(
                &base_cfg,
                seed,
                txs_per_block,
                "cox_style",
                StrategyKind::CoxStyle,
                ticks_per_second,
                page_size,
            ));
        }
    }

    let csv_path = cli.out_dir.join("dual_run_overhead.csv");
    write_csv(&csv_path, &rows)?;
    write_summary_artifacts(&cli.out_dir, &rows)?;
    println!("Wrote {} rows to {}", rows.len(), csv_path.display());

    // Optional MODELED cross-region WAN throughput sweep. Runs the same 3-arm
    // comparison (baseline / SAGE / Cox-style) under increasing simulated
    // inter-region message delay, holding every other knob fixed, so the only
    // variable is the modeled WAN latency. This is a simulator model, not a
    // physically geo-distributed deployment (see docs/EXPERIMENT_FAIRNESS.md and
    // the honest label on tab:wantps in the paper).
    if let Some(wan_delays) = &cli.wan_delays_micros {
        let mut wan_rows = Vec::new();
        for &delay in wan_delays {
            for &seed in &seeds {
                for (mode, strategy) in [
                    ("baseline_poa", StrategyKind::ReconfigOnly),
                    ("sage_dual_run", StrategyKind::Sage),
                    ("cox_style", StrategyKind::CoxStyle),
                ] {
                    let mut row = run_case(
                        &wan_base_with_delay(&base_cfg, delay),
                        seed,
                        cli.wan_txs_per_block,
                        mode,
                        strategy,
                        ticks_per_second,
                        page_size,
                    );
                    // Tag the row's experiment field with the delay so the
                    // aggregator can group by it.
                    row.experiment = format!("wan_throughput_delay{delay}");
                    wan_rows.push(row);
                }
            }
        }
        let wan_csv = cli.out_dir.join("wan_throughput.csv");
        write_csv(&wan_csv, &wan_rows)?;
        write_wan_summary(&cli.out_dir, &wan_rows, wan_delays)?;
        println!("Wrote {} WAN rows to {}", wan_rows.len(), wan_csv.display());
    }
    Ok(())
}

/// Clone the base config with `mean_delay_micros` overridden to the modeled
/// inter-region one-way delay for the WAN sweep.
fn wan_base_with_delay(base_cfg: &SimConfig, delay_micros: u64) -> SimConfig {
    let mut cfg = base_cfg.clone();
    cfg.simulation.mean_delay_micros = delay_micros;
    cfg
}

fn run_case(
    base_cfg: &SimConfig,
    seed: u64,
    txs_per_block: usize,
    mode: &str,
    strategy: StrategyKind,
    ticks_per_second: f64,
    page_size: u64,
) -> OverheadRow {
    let mut cfg = base_cfg.clone();
    cfg.strategy = strategy;
    cfg.simulation.seed = seed;
    cfg.workload.txs_per_block = txs_per_block;
    cfg.simulation.max_events = cfg.simulation.max_events.max(2_000_000);

    let start_proc = read_proc_sample();
    let start = Instant::now();
    let (run_status, metrics) = match Simulation::new(cfg.clone()) {
        Ok(mut sim) => match sim.run() {
            Ok(metrics) => ("ok".to_string(), Some(metrics)),
            Err(e) => (format!("error:{e}"), None),
        },
        Err(e) => (format!("create_error:{e}"), None),
    };
    let wall_seconds = start.elapsed().as_secs_f64().max(1e-9);
    let end_proc = read_proc_sample();

    let finalized_blocks = metrics.as_ref().map_or(0, |m| m.finalized_blocks);
    let max_finalized_height = metrics.as_ref().map_or(0, |m| m.max_finalized_height);
    let committed_txs = max_finalized_height.saturating_mul(txs_per_block as u64);
    let committed_tps_wall = committed_txs as f64 / wall_seconds;
    let offered_tps_model = offered_tps_model(&cfg);
    let shadow_validation_count_model = if strategy.uses_shadow_validation() {
        shadow_validation_count_model(&cfg, max_finalized_height)
    } else {
        0
    };

    let cpu = cpu_percentages(start_proc, end_proc, ticks_per_second, wall_seconds);
    let rss_mb_end = end_proc.map_or(0.0, |p| {
        p.rss_pages.max(0) as f64 * page_size as f64 / 1_048_576.0
    });
    let shadow_time_proxy = if strategy.uses_shadow_validation() {
        // The simulator does not yet expose per-call shadow timers. Attribute the
        // excess wall time above baseline in post-processing; keep this raw proxy
        // as total dual-run processing time for deterministic artifact checks.
        start.elapsed().as_nanos()
    } else {
        0
    };
    let shadow_ns_per_tx = if committed_txs > 0 && shadow_time_proxy > 0 {
        shadow_time_proxy as f64 / committed_txs as f64
    } else {
        0.0
    };

    OverheadRow {
        experiment: "dual_run_overhead".into(),
        mode: mode.into(),
        strategy: format!("{strategy:?}"),
        seed,
        n: cfg.validators.n,
        f: cfg.validators.f,
        txs_per_block,
        offered_tps_model,
        committed_tps_wall,
        wall_seconds,
        cpu_user_pct: cpu.0,
        cpu_system_pct: cpu.1,
        cpu_total_pct: cpu.2,
        rss_mb_end,
        finalized_blocks,
        max_finalized_height,
        committed_txs,
        shadow_validation_count_model,
        shadow_validation_ns_total_proxy: shadow_time_proxy,
        shadow_validation_ns_per_tx_proxy: shadow_ns_per_tx,
        cutover_height: metrics.as_ref().and_then(|m| m.cutover_height),
        migration_success: metrics.as_ref().is_some_and(|m| m.migration_success),
        safety_violation: metrics.as_ref().is_some_and(|m| m.safety_violation),
        generator_saturated: false,
        run_status,
    }
}

fn offered_tps_model(cfg: &SimConfig) -> u64 {
    if cfg.simulation.mean_delay_micros == 0 {
        return 0;
    }
    (cfg.workload.txs_per_block as u64).saturating_mul(1_000_000) / cfg.simulation.mean_delay_micros
}

fn shadow_validation_count_model(cfg: &SimConfig, max_finalized_height: u64) -> u64 {
    let start = cfg.migration.h_d.get();
    let end = max_finalized_height.min(cfg.migration.h_c.get().saturating_sub(1));
    if end < start {
        return 0;
    }
    (end - start + 1).saturating_mul(cfg.validators.n as u64)
}

fn read_proc_sample() -> Option<ProcSample> {
    let stat = fs::read_to_string("/proc/self/stat").ok()?;
    let rparen = stat.rfind(')')?;
    let rest = stat.get(rparen + 2..)?;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    Some(ProcSample {
        // /proc/[pid]/stat fields are 1-indexed; after comm/state removal,
        // utime/stime are at indexes 11/12 and rss at index 21.
        user_ticks: fields.get(11)?.parse().ok()?,
        system_ticks: fields.get(12)?.parse().ok()?,
        rss_pages: fields.get(21)?.parse().ok()?,
    })
}

fn cpu_percentages(
    start: Option<ProcSample>,
    end: Option<ProcSample>,
    ticks_per_second: f64,
    wall_seconds: f64,
) -> (f64, f64, f64) {
    let Some(start) = start else {
        return (0.0, 0.0, 0.0);
    };
    let Some(end) = end else {
        return (0.0, 0.0, 0.0);
    };
    let user = end.user_ticks.saturating_sub(start.user_ticks) as f64 / ticks_per_second;
    let system = end.system_ticks.saturating_sub(start.system_ticks) as f64 / ticks_per_second;
    let user_pct = user / wall_seconds * 100.0;
    let system_pct = system / wall_seconds * 100.0;
    (user_pct, system_pct, user_pct + system_pct)
}

fn clock_ticks_per_second() -> f64 {
    Command::new("getconf")
        .arg("CLK_TCK")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(100.0)
}

fn page_size_bytes() -> u64 {
    Command::new("getconf")
        .arg("PAGESIZE")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(4096)
}

fn write_summary_artifacts(out_dir: &Path, rows: &[OverheadRow]) -> ExperimentResult<()> {
    let results_dir = out_dir
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("results"));
    let table_dir = results_dir.join("tables");
    let figure_dir = results_dir.join("figures");
    fs::create_dir_all(&table_dir)?;
    fs::create_dir_all(&figure_dir)?;

    let summaries = summarize_pairs(rows);
    write_csv(figure_dir.join("dual_run_overhead_summary.csv"), &summaries)?;

    let mut body = String::from("% Auto-generated by run_overhead.rs\n");
    body.push_str("\\begin{tabular}{rrrrr}\n");
    body.push_str("Tx/block & Baseline TPS & SAGE TPS & Cox-style TPS & SAGE vs Cox \\\\\n");
    body.push_str("\\midrule\n");
    for summary in &summaries {
        body.push_str(&format!(
            "{} & {:.0} & {:.0} & {:.0} & {:.1}\\% \\\\\n",
            summary.txs_per_block,
            summary.baseline_tps_mean,
            summary.sage_tps_mean,
            summary.cox_tps_mean,
            summary.sage_vs_cox_tps_pct
        ));
    }
    body.push_str("\\end{tabular}\n");
    fs::write(table_dir.join("dual_run_overhead.tex"), body)?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct WanSummaryRow {
    delay_micros: u64,
    delay_ms: f64,
    trials: usize,
    baseline_tps_mean: f64,
    sage_tps_mean: f64,
    cox_tps_mean: f64,
    sage_vs_cox_tps_pct: f64,
    sage_vs_baseline_tps_pct: f64,
    sage_migrated_all: bool,
    any_safety_violation: bool,
}

/// Summarize the modeled-WAN sweep into a per-delay 3-arm table + CSV. Emits
/// `results/tables/wan_throughput.tex` and `results/figures/wan_throughput_summary.csv`.
fn write_wan_summary(out_dir: &Path, rows: &[OverheadRow], delays: &[u64]) -> ExperimentResult<()> {
    let results_dir = out_dir
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("results"));
    let table_dir = results_dir.join("tables");
    let figure_dir = results_dir.join("figures");
    fs::create_dir_all(&table_dir)?;
    fs::create_dir_all(&figure_dir)?;

    let mut summaries = Vec::new();
    for &delay in delays {
        let tag = format!("wan_throughput_delay{delay}");
        let pick = |mode: &str| -> Vec<&OverheadRow> {
            rows.iter()
                .filter(|r| r.experiment == tag && r.mode == mode && r.run_status == "ok")
                .collect()
        };
        let baseline = pick("baseline_poa");
        let sage = pick("sage_dual_run");
        let cox = pick("cox_style");
        if baseline.is_empty() || sage.is_empty() {
            continue;
        }
        let baseline_tps = mean(baseline.iter().map(|r| r.committed_tps_wall));
        let sage_tps = mean(sage.iter().map(|r| r.committed_tps_wall));
        let cox_tps = mean(cox.iter().map(|r| r.committed_tps_wall));
        summaries.push(WanSummaryRow {
            delay_micros: delay,
            delay_ms: delay as f64 / 1000.0,
            trials: baseline.len().min(sage.len()),
            baseline_tps_mean: baseline_tps,
            sage_tps_mean: sage_tps,
            cox_tps_mean: cox_tps,
            sage_vs_cox_tps_pct: percent_delta(sage_tps, cox_tps),
            sage_vs_baseline_tps_pct: percent_delta(sage_tps, baseline_tps),
            sage_migrated_all: sage.iter().all(|r| r.migration_success),
            any_safety_violation: rows
                .iter()
                .filter(|r| r.experiment == tag)
                .any(|r| r.safety_violation),
        });
    }

    write_csv(figure_dir.join("wan_throughput_summary.csv"), &summaries)?;

    let mut body = String::from("% Auto-generated by run_overhead.rs (modeled WAN sweep)\n");
    body.push_str("\\begin{tabular}{rrrrr}\n");
    body.push_str("Delay (ms) & Baseline TPS & SAGE TPS & Cox-style TPS & SAGE vs Cox \\\\\n");
    body.push_str("\\midrule\n");
    for s in &summaries {
        body.push_str(&format!(
            "{:.0} & {:.0} & {:.0} & {:.0} & {:.1}\\% \\\\\n",
            s.delay_ms, s.baseline_tps_mean, s.sage_tps_mean, s.cox_tps_mean, s.sage_vs_cox_tps_pct
        ));
    }
    body.push_str("\\end{tabular}\n");
    fs::write(table_dir.join("wan_throughput.tex"), body)?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct OverheadSummaryRow {
    txs_per_block: usize,
    trials: usize,
    baseline_tps_mean: f64,
    sage_tps_mean: f64,
    tps_overhead_pct: f64,
    cox_tps_mean: f64,
    sage_vs_cox_tps_pct: f64,
    baseline_cpu_mean: f64,
    sage_cpu_mean: f64,
    cpu_overhead_pct: f64,
    cox_cpu_mean: f64,
    baseline_rss_mb_mean: f64,
    sage_rss_mb_mean: f64,
    cox_rss_mb_mean: f64,
    shadow_validation_count_mean: f64,
}

fn summarize_pairs(rows: &[OverheadRow]) -> Vec<OverheadSummaryRow> {
    let mut txs_values: Vec<usize> = rows.iter().map(|r| r.txs_per_block).collect();
    txs_values.sort_unstable();
    txs_values.dedup();

    txs_values
        .into_iter()
        .filter_map(|txs_per_block| {
            let baseline: Vec<&OverheadRow> = rows
                .iter()
                .filter(|r| {
                    r.txs_per_block == txs_per_block
                        && r.mode == "baseline_poa"
                        && r.run_status == "ok"
                })
                .collect();
            let sage: Vec<&OverheadRow> = rows
                .iter()
                .filter(|r| {
                    r.txs_per_block == txs_per_block
                        && r.mode == "sage_dual_run"
                        && r.run_status == "ok"
                })
                .collect();
            let cox: Vec<&OverheadRow> = rows
                .iter()
                .filter(|r| {
                    r.txs_per_block == txs_per_block
                        && r.mode == "cox_style"
                        && r.run_status == "ok"
                })
                .collect();
            if baseline.is_empty() || sage.is_empty() {
                return None;
            }
            let baseline_tps = mean(baseline.iter().map(|r| r.committed_tps_wall));
            let sage_tps = mean(sage.iter().map(|r| r.committed_tps_wall));
            let cox_tps = mean(cox.iter().map(|r| r.committed_tps_wall));
            let baseline_cpu = mean(baseline.iter().map(|r| r.cpu_total_pct));
            let sage_cpu = mean(sage.iter().map(|r| r.cpu_total_pct));
            let cox_cpu = mean(cox.iter().map(|r| r.cpu_total_pct));
            Some(OverheadSummaryRow {
                txs_per_block,
                trials: baseline.len().min(sage.len()),
                baseline_tps_mean: baseline_tps,
                sage_tps_mean: sage_tps,
                tps_overhead_pct: percent_delta(sage_tps, baseline_tps),
                cox_tps_mean: cox_tps,
                sage_vs_cox_tps_pct: percent_delta(sage_tps, cox_tps),
                baseline_cpu_mean: baseline_cpu,
                sage_cpu_mean: sage_cpu,
                cpu_overhead_pct: percent_delta(sage_cpu, baseline_cpu),
                cox_cpu_mean: cox_cpu,
                baseline_rss_mb_mean: mean(baseline.iter().map(|r| r.rss_mb_end)),
                sage_rss_mb_mean: mean(sage.iter().map(|r| r.rss_mb_end)),
                cox_rss_mb_mean: mean(cox.iter().map(|r| r.rss_mb_end)),
                shadow_validation_count_mean: mean(
                    sage.iter().map(|r| r.shadow_validation_count_model as f64),
                ),
            })
        })
        .collect()
}

fn mean<I>(values: I) -> f64
where
    I: Iterator<Item = f64>,
{
    let mut count = 0usize;
    let mut total = 0.0;
    for value in values {
        count += 1;
        total += value;
    }
    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}

fn percent_delta(new_value: f64, baseline: f64) -> f64 {
    if baseline.abs() < f64::EPSILON {
        0.0
    } else {
        (new_value - baseline) / baseline * 100.0
    }
}
