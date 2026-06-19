//! RQ2: Partition safety — fork rate under adversarial partition.
//! Sweeps partition duration, split ratio across SAGE/HF/SageBlind strategies.
use clap::Parser;
use sage_experiments::common::{load_config, write_csv, write_metadata};
use sage_experiments::error::ExperimentResult;
use sage_sim::{SimConfig, Simulation, StrategyKind};
use sage_stats::{wilson_interval, Interval};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct Rq2Row {
    experiment: String,
    strategy: String,
    seed: u64,
    n: u32,
    f: u64,
    partition_split: u32,
    partition_duration_blocks: u64,
    finalized_blocks: u64,
    safety_violation: bool,
    migration_success: bool,
    max_finalized_height: u64,
    run_status: String,
    disjoint_quorum_windows: u64,
}

#[allow(clippy::too_many_arguments)]
fn run_partition_config(
    base_cfg: &SimConfig,
    strategy: StrategyKind,
    label: &str,
    partition_split: u32,
    partition_duration: u64,
    seeds: &[u64],
    out_dir: &std::path::Path,
    config_path: &std::path::Path,
    config_toml: &str,
) -> Vec<Rq2Row> {
    let mut rows = Vec::new();
    for &seed in seeds {
        let mut cfg = base_cfg.clone();
        cfg.strategy = strategy;
        cfg.simulation.seed = seed;
        cfg.adversary.partition_enabled = true;
        cfg.adversary.partition_split = partition_split;
        cfg.adversary.partition_duration_blocks = partition_duration;
        // Partition begins exactly at the cutover height so the chain reaches
        // h_c cleanly, blind strategies swap to the target engine, and only then
        // does the partition isolate both sides. Starting earlier would stall the
        // single-proposer chain before cutover and the exposure window would be
        // unreachable.
        cfg.adversary.partition_start_height = cfg.migration.h_c.get();

        let status;
        let finalized_blocks;
        let safety_violation;
        let migration_success;
        let max_finalized_height;
        let disjoint_quorum_windows;

        match Simulation::new(cfg) {
            Ok(mut sim) => match sim.run() {
                Ok(metrics) => {
                    status = "ok".to_string();
                    finalized_blocks = metrics.finalized_blocks;
                    safety_violation = metrics.safety_violation;
                    migration_success = metrics.migration_success;
                    max_finalized_height = metrics.max_finalized_height;
                    disjoint_quorum_windows = metrics.disjoint_quorum_windows;
                    let _ = write_metadata(
                        out_dir,
                        &format!(
                            "rq2_{}_s{}_d{}",
                            label.to_lowercase(),
                            partition_split,
                            partition_duration
                        ),
                        config_path,
                        config_toml,
                        seed,
                        "ok",
                        &metrics,
                    );
                }
                Err(e) => {
                    status = format!("error:{}", e);
                    finalized_blocks = 0;
                    safety_violation = false;
                    migration_success = false;
                    max_finalized_height = 0;
                    disjoint_quorum_windows = 0;
                }
            },
            Err(e) => {
                status = format!("create_error:{}", e);
                finalized_blocks = 0;
                safety_violation = false;
                migration_success = false;
                max_finalized_height = 0;
                disjoint_quorum_windows = 0;
            }
        }

        rows.push(Rq2Row {
            experiment: "rq2".to_string(),
            strategy: label.to_string(),
            seed,
            n: base_cfg.validators.n,
            f: base_cfg.validators.f,
            partition_split,
            partition_duration_blocks: partition_duration,
            finalized_blocks,
            safety_violation,
            migration_success,
            max_finalized_height,
            run_status: status,
            disjoint_quorum_windows,
        });
    }
    rows
}

use std::fs;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "config/rq2.toml")]
    config: std::path::PathBuf,
    #[arg(long, default_value = "results/raw")]
    out_dir: std::path::PathBuf,
    #[arg(long, value_delimiter = ',')]
    seeds: Option<Vec<u64>>,
    #[arg(long, default_value = "15")]
    trials: u64,
}

fn main() -> ExperimentResult<()> {
    let cli = Cli::parse();
    let base_cfg = load_config(&cli.config)?;
    let config_toml = fs::read_to_string(&cli.config).unwrap_or_default();
    let seeds = cli.seeds.unwrap_or_else(|| (0..cli.trials).collect());

    let strategies = [
        (StrategyKind::Sage, "Sage"),
        (StrategyKind::HardFork, "HardFork"),
        (StrategyKind::SageBlind, "SageBlind"),
        (StrategyKind::CoxStyle, "CoxStyle"),
    ];

    // Sweep partition configurations
    let splits = [2, 3, 5];
    let durations = [3u64, 6];

    let mut all_rows = Vec::new();
    for (strategy, label) in strategies {
        for &split in &splits {
            for &duration in &durations {
                let rows = run_partition_config(
                    &base_cfg,
                    strategy,
                    label,
                    split,
                    duration,
                    &seeds,
                    &cli.out_dir,
                    &cli.config,
                    &config_toml,
                );
                let safety_count = rows.iter().filter(|r| r.safety_violation).count();
                let total = rows.len() as u64;
                let ci = wilson_interval(safety_count as u64, total, 0.95).unwrap_or(Interval {
                    lower: 0.0,
                    upper: 1.0,
                    confidence: 0.0,
                });
                let rate = if total > 0 {
                    safety_count as f64 / total as f64
                } else {
                    0.0
                };
                println!(
                    "[{}] split={} dur={} n={} fork_rate={:.4} [{:.4}, {:.4}]",
                    label,
                    split,
                    duration,
                    rows.len(),
                    rate,
                    ci.lower,
                    ci.upper
                );
                all_rows.extend(rows);
            }
        }
    }

    let out_path = cli.out_dir.join("rq2_partition_safety.csv");
    write_csv(&out_path, &all_rows)?;
    println!("Wrote {} rows to {}", all_rows.len(), out_path.display());
    Ok(())
}
