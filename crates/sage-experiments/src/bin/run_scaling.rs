//! Validator-count scaling experiment for SAGE.
use clap::Parser;
use sage_experiments::common::{load_config, write_csv, write_metadata};
use sage_experiments::error::ExperimentResult;
use sage_sim::{SimConfig, Simulation, StrategyKind};
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
    #[arg(long, default_value = "5")]
    trials: u64,
    #[arg(long, value_delimiter = ',', default_value = "4,7,10,13,16,20")]
    validators: Vec<u32>,
}

#[derive(Debug, Serialize)]
struct ScalingRow {
    experiment: String,
    strategy: String,
    seed: u64,
    n: u32,
    f: u64,
    finalized_blocks: u64,
    max_finalized_height: u64,
    safety_violation: bool,
    cutover_height: Option<u64>,
    cutover_latency_micros: u64,
    migration_success: bool,
    run_status: String,
}

fn main() -> ExperimentResult<()> {
    let cli = Cli::parse();
    fs::create_dir_all(&cli.out_dir)?;
    let base_cfg = load_config(&cli.config)?;
    let config_toml = fs::read_to_string(&cli.config).unwrap_or_default();
    let seeds = cli.seeds.unwrap_or_else(|| (0..cli.trials).collect());
    let mut rows = Vec::new();

    for &n in &cli.validators {
        let f = ((n.saturating_sub(1)) / 3) as u64;
        if f == 0 || n as u64 <= 3 * f {
            eprintln!("skipping invalid BFT size n={n}, f={f}");
            continue;
        }
        for &seed in &seeds {
            let mut cfg: SimConfig = base_cfg.clone();
            cfg.strategy = StrategyKind::Sage;
            cfg.simulation.seed = seed;
            cfg.validators.n = n;
            cfg.validators.f = f;
            // Event volume grows ~O(n^2) per height (all-to-all messaging), so
            // the event budget must scale with n^2 or large-n runs falsely hit
            // the cap before migrating. Budget = max(existing, c * n^2 * heights).
            let heights = cfg.migration.h_r.get().max(cfg.simulation.max_height);
            let scaled_budget = (n as u64)
                .saturating_mul(n as u64)
                .saturating_mul(heights)
                .saturating_mul(8)
                .max(1_000_000);
            cfg.simulation.max_events = cfg.simulation.max_events.max(scaled_budget);

            let (status, metrics) = match Simulation::new(cfg) {
                Ok(mut sim) => match sim.run() {
                    Ok(metrics) => ("ok".to_string(), Some(metrics)),
                    Err(e) => (format!("error:{e}"), None),
                },
                Err(e) => (format!("create_error:{e}"), None),
            };

            if let Some(metrics) = metrics {
                let _ = write_metadata(
                    &cli.out_dir,
                    &format!("scaling_n{n}"),
                    &cli.config,
                    &config_toml,
                    seed,
                    &status,
                    &metrics,
                );
                rows.push(ScalingRow {
                    experiment: "scaling".into(),
                    strategy: "Sage".into(),
                    seed,
                    n,
                    f,
                    finalized_blocks: metrics.finalized_blocks,
                    max_finalized_height: metrics.max_finalized_height,
                    safety_violation: metrics.safety_violation,
                    cutover_height: metrics.cutover_height,
                    cutover_latency_micros: metrics.cutover_latency_micros.unwrap_or(0),
                    migration_success: metrics.migration_success,
                    run_status: status,
                });
            } else {
                rows.push(ScalingRow {
                    experiment: "scaling".into(),
                    strategy: "Sage".into(),
                    seed,
                    n,
                    f,
                    finalized_blocks: 0,
                    max_finalized_height: 0,
                    safety_violation: false,
                    cutover_height: None,
                    cutover_latency_micros: 0,
                    migration_success: false,
                    run_status: status,
                });
            }
        }
    }

    let out_path = cli.out_dir.join("scaling.csv");
    write_csv(&out_path, &rows)?;
    println!("Wrote {} rows to {}", rows.len(), out_path.display());
    Ok(())
}
