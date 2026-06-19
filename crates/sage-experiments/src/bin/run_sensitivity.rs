//! Network delay sensitivity experiment: sweeps mean_delay_micros and
//! measures cutover latency, dual-run overhead, and throughput for SAGE.
//! Supports RQ5 of the journal experiment plan.
use clap::Parser;
use sage_experiments::common::{load_config, write_csv};
use sage_experiments::error::ExperimentResult;
use sage_sim::{SimConfig, Simulation, StrategyKind};
use sage_stats::mean_ci_bootstrap;
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
    /// Comma-separated delay values in microseconds
    #[arg(
        long,
        value_delimiter = ',',
        default_value = "40000,80000,160000,320000"
    )]
    delays: Vec<u64>,
}

#[derive(Debug, Serialize)]
struct SensitivityRow {
    experiment: String,
    strategy: String,
    seed: u64,
    mean_delay_micros: u64,
    finalized_blocks: u64,
    max_finalized_height: u64,
    safety_violation: bool,
    cutover_height: Option<u64>,
    cutover_latency_micros: u64,
    migration_success: bool,
    run_status: String,
}

fn run_for_delay(
    base_cfg: &SimConfig,
    delay_micros: u64,
    seeds: &[u64],
    _out_dir: &std::path::Path,
    _config_toml: &str,
) -> Vec<SensitivityRow> {
    let mut rows = Vec::new();
    for &seed in seeds {
        let mut cfg = base_cfg.clone();
        cfg.strategy = StrategyKind::Sage;
        cfg.simulation.seed = seed;
        cfg.simulation.mean_delay_micros = delay_micros;

        let status;
        let finalized_blocks;
        let max_finalized_height;
        let safety_violation;
        let cutover_height;
        let cutover_latency_micros;
        let migration_success;

        match Simulation::new(cfg.clone()) {
            Ok(mut sim) => match sim.run() {
                Ok(metrics) => {
                    status = "ok".into();
                    finalized_blocks = metrics.finalized_blocks;
                    max_finalized_height = metrics.max_finalized_height;
                    safety_violation = metrics.safety_violation;
                    cutover_height = metrics.cutover_height;
                    cutover_latency_micros = metrics.cutover_latency_micros.unwrap_or(0);
                    migration_success = metrics.migration_success;
                }
                Err(e) => {
                    status = format!("error:{}", e);
                    finalized_blocks = 0;
                    max_finalized_height = 0;
                    safety_violation = false;
                    cutover_height = None;
                    cutover_latency_micros = 0;
                    migration_success = false;
                }
            },
            Err(e) => {
                status = format!("create_error:{}", e);
                finalized_blocks = 0;
                max_finalized_height = 0;
                safety_violation = false;
                cutover_height = None;
                cutover_latency_micros = 0;
                migration_success = false;
            }
        }

        let row = SensitivityRow {
            experiment: "sensitivity".into(),
            strategy: "Sage".into(),
            seed,
            mean_delay_micros: delay_micros,
            finalized_blocks,
            max_finalized_height,
            safety_violation,
            cutover_height,
            cutover_latency_micros,
            migration_success,
            run_status: status,
        };
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

    let mut all_rows = Vec::new();
    for &delay in &cli.delays {
        let rows = run_for_delay(&base_cfg, delay, &seeds, &cli.out_dir, &config_toml);
        let cutover_lat: Vec<f64> = rows
            .iter()
            .filter(|r| r.migration_success)
            .map(|r| r.cutover_latency_micros as f64)
            .collect();
        let ci = mean_ci_bootstrap(&cutover_lat, 0.95, 42, 500).unwrap_or(sage_stats::Interval {
            lower: 0.0,
            upper: 0.0,
            confidence: 0.0,
        });
        let mean = if !cutover_lat.is_empty() {
            cutover_lat.iter().sum::<f64>() / cutover_lat.len() as f64
        } else {
            0.0
        };
        println!(
            "[delay={}us] n={} cutover_lat_us={:.1} [{:.1}, {:.1}]",
            delay,
            rows.len(),
            mean,
            ci.lower,
            ci.upper,
        );
        all_rows.extend(rows);
    }

    let out_path = cli.out_dir.join("sensitivity.csv");
    write_csv(&out_path, &all_rows)?;
    println!("Wrote {} rows to {}", all_rows.len(), out_path.display());
    Ok(())
}
