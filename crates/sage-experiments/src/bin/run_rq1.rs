//! RQ1: Migration cost against baselines.
//! Runs SAGE, STW, HF, RC strategies and compares downtime/finality gap.
use clap::Parser;
use sage_experiments::common::{load_config, write_csv, write_metadata};
use sage_experiments::error::ExperimentResult;
use sage_sim::{SimConfig, Simulation, StrategyKind};
use sage_stats::{holm_bonferroni, mann_whitney_u, mean_ci_bootstrap, wilson_interval, Interval};
use serde::Serialize;
use std::fs;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "config/rq1.toml")]
    config: std::path::PathBuf,
    #[arg(long, default_value = "results/raw")]
    out_dir: std::path::PathBuf,
    #[arg(long, value_delimiter = ',')]
    seeds: Option<Vec<u64>>,
    #[arg(long, default_value = "30")]
    trials: u64,
}

#[derive(Debug, Serialize, Clone)]
struct Rq1Row {
    strategy: String,
    seed: u64,
    run_status: String,
    finalized_blocks: u64,
    max_finalized_height: u64,
    safety_violation: Option<bool>,
    downtime_micros: u64,
    finality_gap: u64,
    cutover_latency_micros: u64,
}

/// One SAGE-vs-baseline statistical comparison on the downtime metric.
#[derive(Debug, Serialize, Clone)]
struct Rq1StatRow {
    metric: String,
    baseline: String,
    sage_mean: f64,
    baseline_mean: f64,
    u_statistic: f64,
    p_value: f64,
    rank_biserial: f64,
    holm_adjusted_alpha: f64,
    reject_null: bool,
    n_sage: usize,
    n_baseline: usize,
}

fn run_strategy(
    base_cfg: &SimConfig,
    strategy: StrategyKind,
    label: &str,
    seeds: &[u64],
    out_dir: &std::path::Path,
    config_path: &std::path::Path,
    config_toml: &str,
) -> Vec<Rq1Row> {
    let mut rows = Vec::new();
    for &seed in seeds {
        let mut cfg = base_cfg.clone();
        cfg.strategy = strategy;
        cfg.simulation.seed = seed;
        match Simulation::new(cfg.clone()) {
            Ok(mut sim) => match sim.run() {
                Ok(metrics) => {
                    // Observed service interruption: the largest gap between
                    // consecutive finalization timestamps, measured by the
                    // recorder from real sim-time. Not derived from the flag.
                    let downtime = metrics.max_finalization_gap_micros;
                    // Finality gap in blocks: heights the target never reached.
                    let gap = cfg
                        .simulation
                        .max_height
                        .saturating_sub(metrics.max_finalized_height);
                    let row = Rq1Row {
                        strategy: label.to_string(),
                        seed,
                        run_status: "completed".into(),
                        finalized_blocks: metrics.finalized_blocks,
                        max_finalized_height: metrics.max_finalized_height,
                        safety_violation: Some(metrics.safety_violation),
                        downtime_micros: downtime,
                        finality_gap: gap,
                        cutover_latency_micros: metrics.cutover_latency_micros.unwrap_or(0),
                    };
                    let _ = write_metadata(
                        out_dir,
                        &format!("rq1_{}", label.to_lowercase()),
                        config_path,
                        config_toml,
                        seed,
                        "completed",
                        &metrics,
                    );
                    rows.push(row);
                }
                Err(e) => {
                    eprintln!("[{}] seed={} error={:?}", label, seed, e);
                    rows.push(Rq1Row {
                        strategy: label.to_string(),
                        seed,
                        run_status: format!("error: {:?}", e),
                        finalized_blocks: 0,
                        max_finalized_height: 0,
                        safety_violation: None,
                        downtime_micros: 0,
                        finality_gap: cfg.simulation.max_height,
                        cutover_latency_micros: 0,
                    });
                }
            },
            Err(e) => {
                eprintln!("[{}] seed={} create error={:?}", label, seed, e);
                rows.push(Rq1Row {
                    strategy: label.to_string(),
                    seed,
                    run_status: format!("create_error: {:?}", e),
                    finalized_blocks: 0,
                    max_finalized_height: 0,
                    safety_violation: None,
                    downtime_micros: 0,
                    finality_gap: cfg.simulation.max_height,
                    cutover_latency_micros: 0,
                });
            }
        }
    }
    rows
}

fn main() -> ExperimentResult<()> {
    let cli = Cli::parse();
    fs::create_dir_all(&cli.out_dir).expect("create out dir");

    let base_cfg = load_config(&cli.config)?;
    let config_toml = fs::read_to_string(&cli.config).unwrap_or_default();
    let seeds = cli
        .seeds
        .unwrap_or_else(|| (0..cli.trials).collect::<Vec<_>>());
    let strategies = [
        (StrategyKind::Sage, "Sage"),
        (StrategyKind::StopTheWorld, "StopTheWorld"),
        (StrategyKind::HardFork, "HardFork"),
        (StrategyKind::ReconfigOnly, "ReconfigOnly"),
        (StrategyKind::CoxStyle, "CoxStyle"),
    ];

    let mut all_rows = Vec::new();
    for (strategy, label) in strategies {
        let rows = run_strategy(
            &base_cfg,
            strategy,
            label,
            &seeds,
            &cli.out_dir,
            &cli.config,
            &config_toml,
        );
        print_summary(label, &rows);
        all_rows.extend(rows);
    }

    let out_path = cli.out_dir.join("rq1_migration_cost.csv");
    write_csv(&out_path, &all_rows)?;
    println!("Wrote {} rows to {}", all_rows.len(), out_path.display());
    let incomplete = all_rows
        .iter()
        .filter(|r| r.run_status != "completed" || r.safety_violation.is_none())
        .count();
    if incomplete > 0 {
        return Err(sage_experiments::error::ExperimentError::Simulation(format!(
            "RQ1 has {incomplete} incomplete trial(s); raw rows were written, statistics were not computed"
        )));
    }

    // SAGE-vs-baseline statistics on the observed downtime metric.
    // Mann-Whitney U (non-parametric, no normality assumption) with the
    // rank-biserial effect size, then Holm-Bonferroni across the family of
    // baseline comparisons to control the family-wise error rate.
    let stats_rows = compute_downtime_stats(&all_rows);
    let stats_path = cli.out_dir.join("rq1_stats.csv");
    write_csv(&stats_path, &stats_rows)?;
    println!(
        "Wrote {} stat rows to {}",
        stats_rows.len(),
        stats_path.display()
    );
    for s in &stats_rows {
        println!(
            "[stat] SAGE vs {:<12} downtime: sage_mean={:.1} base_mean={:.1} p={:.4} \
             rb={:+.3} holm_alpha={:.4} reject={}",
            s.baseline,
            s.sage_mean,
            s.baseline_mean,
            s.p_value,
            s.rank_biserial,
            s.holm_adjusted_alpha,
            s.reject_null
        );
    }
    Ok(())
}

/// Compute SAGE-vs-each-baseline Mann-Whitney comparisons on observed downtime,
/// then apply Holm-Bonferroni across the comparison family.
fn compute_downtime_stats(rows: &[Rq1Row]) -> Vec<Rq1StatRow> {
    let downtime_of = |label: &str| -> Vec<f64> {
        rows.iter()
            .filter(|r| r.strategy == label && r.run_status == "completed")
            .map(|r| r.downtime_micros as f64)
            .collect()
    };
    let mean = |v: &[f64]| -> f64 {
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<f64>() / v.len() as f64
        }
    };

    let sage = downtime_of("Sage");
    let sage_mean = mean(&sage);
    let baselines = ["StopTheWorld", "HardFork", "ReconfigOnly", "CoxStyle"];

    // First pass: collect test results and p-values in baseline order.
    let mut partial: Vec<(String, f64, Vec<f64>, sage_stats::MannWhitneyResult)> = Vec::new();
    let mut p_values: Vec<f64> = Vec::new();
    for base in baselines {
        let b = downtime_of(base);
        if sage.is_empty() || b.is_empty() {
            continue;
        }
        match mann_whitney_u(&sage, &b) {
            Ok(res) => {
                p_values.push(res.p_value);
                partial.push((base.to_string(), mean(&b), b, res));
            }
            Err(e) => eprintln!("[stat] mann_whitney error for {}: {:?}", base, e),
        }
    }

    let decisions = holm_bonferroni(&p_values, 0.05);
    partial
        .into_iter()
        .enumerate()
        .map(|(i, (base, base_mean, b, res))| {
            let dec = decisions.get(i);
            Rq1StatRow {
                metric: "downtime_micros".to_string(),
                baseline: base,
                sage_mean,
                baseline_mean: base_mean,
                u_statistic: res.u_statistic,
                p_value: res.p_value,
                rank_biserial: res.rank_biserial,
                holm_adjusted_alpha: dec.map(|d| d.adjusted_alpha).unwrap_or(0.05),
                reject_null: dec.map(|d| d.reject).unwrap_or(false),
                n_sage: sage.len(),
                n_baseline: b.len(),
            }
        })
        .collect()
}

fn print_summary(label: &str, rows: &[Rq1Row]) {
    let completed: Vec<_> = rows
        .iter()
        .filter(|r| r.run_status == "completed" && r.safety_violation.is_some())
        .collect();
    let finalized: Vec<f64> = completed
        .iter()
        .map(|r| r.finalized_blocks as f64)
        .collect();
    let safety_succ = completed
        .iter()
        .filter(|r| r.safety_violation == Some(true))
        .count() as u64;

    let ci = mean_ci_bootstrap(&finalized, 0.95, 42, 1000).unwrap_or(Interval {
        lower: 0.0,
        upper: 0.0,
        confidence: 0.0,
    });
    let safety_rate = if completed.is_empty() {
        0.0
    } else {
        safety_succ as f64 / completed.len() as f64
    };
    let safety_ci =
        wilson_interval(safety_succ, completed.len() as u64, 0.95).unwrap_or(Interval {
            lower: 0.0,
            upper: 1.0,
            confidence: 0.0,
        });

    println!(
        "[{}] n={} mean_finalized={:.1} [{:.1}, {:.1}] safety_rate={:.3} [{:.3}, {:.3}]",
        label,
        completed.len(),
        ci.lower + (ci.upper - ci.lower) / 2.0,
        ci.lower,
        ci.upper,
        safety_rate,
        safety_ci.lower,
        safety_ci.upper,
    );
}
