//! Safety experiment: runs SAGE, HardFork, and SageBlind strategies
//! under adversarial partition and reports fork/safety-violation rates.
//! Uses config/safety.toml which is tuned for fork production (n=6, f=1).
use clap::Parser;
use sage_experiments::common::{load_config, write_csv, write_metadata};
use sage_experiments::error::ExperimentResult;
use sage_sim::{SimConfig, Simulation, StrategyKind};
use sage_stats::{wilson_interval, Interval};
use serde::Serialize;
use std::fs;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "config/safety.toml")]
    config: std::path::PathBuf,
    #[arg(long, default_value = "results/raw")]
    out_dir: std::path::PathBuf,
    #[arg(long, default_value = "30")]
    trials: u64,
    #[arg(long, value_delimiter = ',')]
    seeds: Option<Vec<u64>>,
}

#[derive(Debug, Serialize)]
struct SafetyRow {
    strategy: String,
    seed: u64,
    run_status: String,
    finalized_blocks: u64,
    safety_violation: bool,
    migration_success: bool,
    max_finalized_height: u64,
    disjoint_quorum_windows: u64,
}

fn run_trials(
    base_cfg: &SimConfig,
    strategy: StrategyKind,
    label: &str,
    seeds: &[u64],
    out_dir: &std::path::Path,
    config_path: &std::path::Path,
    config_toml: &str,
) -> Vec<SafetyRow> {
    let mut rows = Vec::new();
    for &seed in seeds {
        let mut cfg = base_cfg.clone();
        cfg.strategy = strategy;
        cfg.simulation.seed = seed;
        let status;
        let finalized_blocks;
        let safety_violation;
        let migration_success;
        let max_finalized_height;
        let disjoint_quorum_windows;

        match Simulation::new(cfg.clone()) {
            Ok(mut sim) => match sim.run() {
                Ok(metrics) => {
                    status = "completed".into();
                    finalized_blocks = metrics.finalized_blocks;
                    safety_violation = metrics.safety_violation;
                    migration_success = metrics.migration_success;
                    max_finalized_height = metrics.max_finalized_height;
                    disjoint_quorum_windows = metrics.disjoint_quorum_windows;
                    let _ = write_metadata(
                        out_dir,
                        &format!("safety_{}", label.to_lowercase()),
                        config_path,
                        config_toml,
                        seed,
                        "completed",
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

        rows.push(SafetyRow {
            strategy: label.to_string(),
            seed,
            run_status: status,
            finalized_blocks,
            safety_violation,
            migration_success,
            max_finalized_height,
            disjoint_quorum_windows,
        });
    }
    rows
}

fn main() -> ExperimentResult<()> {
    let cli = Cli::parse();
    fs::create_dir_all(&cli.out_dir)?;
    let base_cfg = load_config(&cli.config)?;
    let config_toml = fs::read_to_string(&cli.config).unwrap_or_default();
    let seeds = cli
        .seeds
        .unwrap_or_else(|| (0..cli.trials).collect::<Vec<u64>>());

    let strategies = [
        (StrategyKind::Sage, "Sage"),
        (StrategyKind::HardFork, "HardFork"),
        (StrategyKind::SageBlind, "SageBlind"),
    ];

    let mut all_rows = Vec::new();
    for (strategy, label) in strategies {
        let rows = run_trials(
            &base_cfg,
            strategy,
            label,
            &seeds,
            &cli.out_dir,
            &cli.config,
            &config_toml,
        );
        let safety_count = rows.iter().filter(|r| r.safety_violation).count() as u64;
        let total = rows.len() as u64;
        let rate = if total > 0 {
            safety_count as f64 / total as f64
        } else {
            0.0
        };
        let ci = wilson_interval(safety_count, total, 0.95).unwrap_or(Interval {
            lower: 0.0,
            upper: 1.0,
            confidence: 0.0,
        });
        println!(
            "[{}] n={} safety_rate={:.4} [{:.4}, {:.4}]",
            label,
            rows.len(),
            rate,
            ci.lower,
            ci.upper
        );
        all_rows.extend(rows);
    }

    let out_path = cli.out_dir.join("safety_partition.csv");
    write_csv(&out_path, &all_rows)?;
    println!("Wrote {} rows to {}", all_rows.len(), out_path.display());
    Ok(())
}
