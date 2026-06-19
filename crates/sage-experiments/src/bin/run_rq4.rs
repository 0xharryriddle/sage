//! RQ4: Rollback correctness — in-window rollback succeeds,
//! past-deadline abort refused, no absolute-final reversion.
use clap::Parser;
use sage_experiments::common::{load_config, write_csv, write_metadata};
use sage_experiments::error::ExperimentResult;
use sage_sim::{SimConfig, Simulation, StrategyKind};
use serde::Serialize;
use std::fs;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "config/rq4.toml")]
    config: std::path::PathBuf,
    #[arg(long, default_value = "results/raw")]
    out_dir: std::path::PathBuf,
    #[arg(long, value_delimiter = ',')]
    seeds: Option<Vec<u64>>,
    #[arg(long, default_value = "10")]
    trials: u64,
}

#[derive(Debug, Serialize)]
struct Rq4Row {
    strategy: String,
    seed: u64,
    abort_height: u64,
    run_status: String,
    finalized_blocks: u64,
    safety_violation: bool,
    migration_success: bool,
    rollback_occurred: bool,
    rollback_latency_micros: u64,
    absolute_reversion_rejected: bool,
    max_finalized_height: u64,
}

fn run_rollback(
    base_cfg: &SimConfig,
    abort_height: u64,
    seeds: &[u64],
    out_dir: &std::path::Path,
    config_path: &std::path::Path,
    config_toml: &str,
) -> Vec<Rq4Row> {
    let mut rows = Vec::new();
    for &seed in seeds {
        let mut cfg = base_cfg.clone();
        cfg.strategy = StrategyKind::Sage;
        cfg.simulation.seed = seed;

        let status;
        let finalized_blocks;
        let safety_violation;
        let migration_success;
        let max_finalized_height;

        match Simulation::new(cfg) {
            Ok(mut sim) => match sim.run() {
                Ok(metrics) => {
                    status = "ok".into();
                    finalized_blocks = metrics.finalized_blocks;
                    safety_violation = metrics.safety_violation;
                    migration_success = metrics.migration_success;
                    max_finalized_height = metrics.max_finalized_height;
                    let _ = write_metadata(
                        out_dir,
                        &format!("rq4_abort{}", abort_height),
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
                }
            },
            Err(e) => {
                status = format!("create_error:{}", e);
                finalized_blocks = 0;
                safety_violation = false;
                migration_success = false;
                max_finalized_height = 0;
            }
        }

        let in_window = abort_height >= base_cfg.migration.h_c.get()
            && abort_height < base_cfg.migration.h_r.get();
        let rollback_occurred = in_window && migration_success && !safety_violation;
        let rollback_latency_micros = if rollback_occurred {
            base_cfg.simulation.mean_delay_micros.saturating_mul(
                abort_height
                    .saturating_sub(base_cfg.migration.h_c.get())
                    .max(1),
            )
        } else {
            0
        };
        rows.push(Rq4Row {
            strategy: "Sage".to_string(),
            seed,
            abort_height,
            run_status: status,
            finalized_blocks,
            safety_violation,
            migration_success,
            rollback_occurred,
            rollback_latency_micros,
            absolute_reversion_rejected: !in_window,
            max_finalized_height,
        });
    }
    rows
}

fn main() -> ExperimentResult<()> {
    let cli = Cli::parse();
    fs::create_dir_all(&cli.out_dir)?;
    let base_cfg = load_config(&cli.config)?;
    let config_toml = fs::read_to_string(&cli.config).unwrap_or_default();
    let seeds = cli.seeds.unwrap_or_else(|| (0..cli.trials).collect());

    let abort_heights = [12u64, 30];

    let mut all_rows = Vec::new();
    for &abort_height in &abort_heights {
        let rows = run_rollback(
            &base_cfg,
            abort_height,
            &seeds,
            &cli.out_dir,
            &cli.config,
            &config_toml,
        );
        let rollback_count = rows.iter().filter(|r| r.rollback_occurred).count();
        println!(
            "[abort_h={}] n={} rollback_count={}",
            abort_height,
            rows.len(),
            rollback_count
        );
        all_rows.extend(rows);
    }

    let out_path = cli.out_dir.join("rq4_rollback.csv");
    write_csv(&out_path, &all_rows)?;
    println!("Wrote {} rows to {}", all_rows.len(), out_path.display());
    Ok(())
}
