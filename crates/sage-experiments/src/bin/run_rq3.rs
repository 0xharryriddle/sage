//! RQ3: Kappa ablation — how shadow-validation threshold affects
//! unsafe cutover probability and migration length.
use clap::Parser;
use sage_experiments::common::{load_config, write_csv, write_metadata};
use sage_experiments::error::ExperimentResult;
use sage_sim::{SimConfig, Simulation, StrategyKind};
use sage_stats::{wilson_interval, Interval};
use serde::Serialize;
use std::fs;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "config/rq3.toml")]
    config: std::path::PathBuf,
    #[arg(long, default_value = "results/raw")]
    out_dir: std::path::PathBuf,
    #[arg(long, value_delimiter = ',')]
    seeds: Option<Vec<u64>>,
    #[arg(long, default_value = "5")]
    trials: u64,
    /// Comma-separated kappa values to sweep
    #[arg(long, value_delimiter = ',', default_value = "1,2,4,8,16")]
    kappa: Vec<u64>,
}

#[derive(Debug, Serialize)]
struct Rq3Row {
    strategy: String,
    seed: u64,
    kappa: u64,
    run_status: String,
    finalized_blocks: u64,
    safety_violation: bool,
    migration_success: bool,
    cutover_height: Option<u64>,
    cutover_latency_micros: u64,
    max_finalized_height: u64,
}

fn run_kappa(
    base_cfg: &SimConfig,
    kappa: u64,
    seeds: &[u64],
    out_dir: &std::path::Path,
    config_path: &std::path::Path,
    config_toml: &str,
) -> Vec<Rq3Row> {
    let mut rows = Vec::new();
    for &seed in seeds {
        let mut cfg = base_cfg.clone();
        cfg.strategy = StrategyKind::Sage;
        cfg.simulation.seed = seed;
        cfg.migration.kappa = kappa;

        match Simulation::new(cfg) {
            Ok(mut sim) => match sim.run() {
                Ok(metrics) => {
                    let row = Rq3Row {
                        strategy: "Sage".to_string(),
                        seed,
                        kappa,
                        run_status: "completed".into(),
                        finalized_blocks: metrics.finalized_blocks,
                        safety_violation: metrics.safety_violation,
                        migration_success: metrics.migration_success,
                        cutover_height: metrics.cutover_height,
                        cutover_latency_micros: metrics.cutover_latency_micros.unwrap_or(0),
                        max_finalized_height: metrics.max_finalized_height,
                    };
                    let _ = write_metadata(
                        out_dir,
                        &format!("rq3_k{}", kappa),
                        config_path,
                        config_toml,
                        seed,
                        "completed",
                        &metrics,
                    );
                    rows.push(row);
                }
                Err(e) => {
                    eprintln!("[kappa={}] seed={} error={:?}", kappa, seed, e);
                    rows.push(Rq3Row {
                        strategy: "Sage".to_string(),
                        seed,
                        kappa,
                        run_status: format!("error: {:?}", e),
                        finalized_blocks: 0,
                        safety_violation: false,
                        migration_success: false,
                        cutover_height: None,
                        cutover_latency_micros: 0,
                        max_finalized_height: 0,
                    });
                }
            },
            Err(e) => {
                eprintln!("[kappa={}] seed={} create error={:?}", kappa, seed, e);
            }
        }
    }
    rows
}

fn main() -> ExperimentResult<()> {
    let cli = Cli::parse();
    let base_cfg = load_config(&cli.config)?;
    let config_toml = fs::read_to_string(&cli.config).unwrap_or_default();
    let seeds = cli
        .seeds
        .unwrap_or_else(|| (0..cli.trials).collect::<Vec<_>>());

    let mut all_rows = Vec::new();
    for &kappa in &cli.kappa {
        let rows = run_kappa(
            &base_cfg,
            kappa,
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
            "[kappa={}] n={} unsafe_rate={:.4} [{:.4}, {:.4}]",
            kappa,
            rows.len(),
            rate,
            ci.lower,
            ci.upper
        );
        all_rows.extend(rows);
    }

    let out_path = cli.out_dir.join("rq3_kappa_ablation.csv");
    write_csv(&out_path, &all_rows)?;
    println!("Wrote {} rows to {}", all_rows.len(), out_path.display());
    Ok(())
}
