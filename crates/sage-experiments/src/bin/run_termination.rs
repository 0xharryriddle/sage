//! Termination experiment: sweeps partition duration to find
//! maximum partition length where strategies remain live.
//! Supports RQ2 and RQ5 of the journal experiment plan.
use clap::Parser;
use sage_experiments::common::{load_config, write_csv, write_metadata};
use sage_experiments::error::ExperimentResult;
use sage_sim::{SimConfig, Simulation, StrategyKind};
use sage_stats::{wilson_interval, Interval};
use serde::Serialize;
use std::fs;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "config/rq2.toml")]
    config: std::path::PathBuf,
    #[arg(long, default_value = "results/raw")]
    out_dir: std::path::PathBuf,
    #[arg(long, value_delimiter = ',')]
    seeds: Option<Vec<u64>>,
    #[arg(long, default_value = "5")]
    trials: u64,
    /// Comma-separated partition durations to sweep
    #[arg(long, value_delimiter = ',', default_value = "2,4,6,8,12,16")]
    durations: Vec<u64>,
}

#[derive(Debug, Serialize)]
struct TerminationRow {
    experiment: String,
    strategy: String,
    seed: u64,
    partition_duration_blocks: u64,
    finalized_blocks: u64,
    safety_violation: bool,
    liveness_stall: bool,
    max_finalized_height: u64,
    run_status: String,
}

#[allow(clippy::too_many_arguments)]
fn run_for_duration(
    base_cfg: &SimConfig,
    strategy: StrategyKind,
    label: &str,
    duration: u64,
    seeds: &[u64],
    out_dir: &std::path::Path,
    config_path: &std::path::Path,
    config_toml: &str,
) -> Vec<TerminationRow> {
    let mut rows = Vec::new();
    for &seed in seeds {
        let mut cfg = base_cfg.clone();
        cfg.strategy = strategy;
        cfg.simulation.seed = seed;
        cfg.adversary.partition_enabled = true;
        cfg.adversary.partition_duration_blocks = duration;
        cfg.adversary.partition_start_height = cfg.migration.h_c.get().saturating_sub(1);
        cfg.adversary.partition_split = cfg.validators.n / 2;

        let status;
        let finalized_blocks;
        let safety_violation;
        let max_finalized_height;
        let liveness_stall;

        match Simulation::new(cfg.clone()) {
            Ok(mut sim) => match sim.run() {
                Ok(metrics) => {
                    status = "ok".into();
                    finalized_blocks = metrics.finalized_blocks;
                    safety_violation = metrics.safety_violation;
                    max_finalized_height = metrics.max_finalized_height;
                    liveness_stall = metrics.max_finalized_height < cfg.simulation.max_height / 2;
                }
                Err(e) => {
                    let msg = format!("{}", e);
                    status = if msg.contains("exceeded") {
                        "timeout_max_events".into()
                    } else {
                        format!("error:{}", msg)
                    };
                    finalized_blocks = 0;
                    safety_violation = false;
                    max_finalized_height = 0;
                    liveness_stall = status.contains("timeout");
                }
            },
            Err(e) => {
                status = format!("create_error:{}", e);
                finalized_blocks = 0;
                safety_violation = false;
                max_finalized_height = 0;
                liveness_stall = true;
            }
        }

        let row = TerminationRow {
            experiment: "termination".into(),
            strategy: label.into(),
            seed,
            partition_duration_blocks: duration,
            finalized_blocks,
            safety_violation,
            liveness_stall,
            max_finalized_height,
            run_status: status,
        };
        let _ = write_metadata(
            out_dir,
            &format!("term_{}_d{}", label.to_lowercase(), duration),
            config_path,
            config_toml,
            seed,
            &row.run_status,
            &sage_sim::RunMetrics::default(),
        );
        rows.push(row);
    }
    rows
}

fn main() -> ExperimentResult<()> {
    let cli = Cli::parse();
    fs::create_dir_all(&cli.out_dir)?;
    let base_cfg = load_config(&cli.config)?;
    let config_toml = fs::read_to_string(&cli.config).unwrap_or_default();
    let seeds = cli.seeds.unwrap_or_else(|| (0..cli.trials).collect());

    let strategies = [
        (StrategyKind::Sage, "Sage"),
        (StrategyKind::HardFork, "HardFork"),
    ];

    let mut all_rows = Vec::new();
    for (strategy, label) in strategies {
        for &duration in &cli.durations {
            let rows = run_for_duration(
                &base_cfg,
                strategy,
                label,
                duration,
                &seeds,
                &cli.out_dir,
                &cli.config,
                &config_toml,
            );
            let stall_count = rows.iter().filter(|r| r.liveness_stall).count() as u64;
            let total = rows.len() as u64;
            let ci = wilson_interval(stall_count, total, 0.95).unwrap_or(Interval {
                lower: 0.0,
                upper: 1.0,
                confidence: 0.0,
            });
            println!(
                "[{}] dur={} n={} stall_rate={:.4} [{:.4}, {:.4}]",
                label,
                duration,
                total,
                stall_count as f64 / total.max(1) as f64,
                ci.lower,
                ci.upper
            );
            all_rows.extend(rows);
        }
    }

    let out_path = cli.out_dir.join("termination.csv");
    write_csv(&out_path, &all_rows)?;
    println!("Wrote {} rows to {}", all_rows.len(), out_path.display());
    Ok(())
}
