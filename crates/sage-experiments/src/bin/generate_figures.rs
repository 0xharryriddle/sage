//! Generate PGFPlots-compatible figure data from raw experiment CSVs.
use clap::Parser;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "results/raw")]
    raw_dir: PathBuf,
    #[arg(long, default_value = "results/figures")]
    out_dir: PathBuf,
    #[arg(long, default_value_t = false)]
    allow_missing: bool,
}

#[derive(Debug, Deserialize)]
struct Rq1Row {
    strategy: String,
    cutover_latency_micros: u64,
    downtime_micros: u64,
}

#[derive(Debug, Deserialize)]
struct Rq2Row {
    strategy: String,
    partition_split: Option<u32>,
    partition_duration_blocks: Option<u64>,
    safety_violation: bool,
    disjoint_quorum_windows: u64,
}

#[derive(Debug, Deserialize)]
struct Rq3Row {
    kappa: u64,
    safety_violation: bool,
    migration_success: bool,
    cutover_latency_micros: u64,
}

#[derive(Debug, Deserialize)]
struct Rq4Row {
    abort_height: u64,
    rollback_occurred: bool,
    rollback_latency_micros: u64,
    absolute_reversion_rejected: bool,
}

#[derive(Debug, Deserialize)]
struct TerminationRow {
    strategy: String,
    partition_duration_blocks: u64,
    liveness_stall: bool,
    safety_violation: bool,
}

#[derive(Debug, Deserialize)]
struct SensitivityRow {
    mean_delay_micros: u64,
    cutover_latency_micros: u64,
    migration_success: bool,
    finalized_blocks: u64,
}

#[derive(Debug, Deserialize)]
struct ScalingRow {
    n: u32,
    cutover_latency_micros: u64,
    migration_success: bool,
    finalized_blocks: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    fs::create_dir_all(&cli.out_dir)?;

    generate_if_exists(
        &cli.raw_dir,
        &cli.out_dir,
        "rq1_migration_cost.csv",
        cli.allow_missing,
        write_rq1_cdf,
    )?;
    generate_if_exists(
        &cli.raw_dir,
        &cli.out_dir,
        "rq1_migration_cost.csv",
        cli.allow_missing,
        write_rq1_downtime,
    )?;
    generate_if_exists(
        &cli.raw_dir,
        &cli.out_dir,
        "rq2_partition_safety.csv",
        cli.allow_missing,
        write_rq2_fork_rate,
    )?;
    generate_if_exists(
        &cli.raw_dir,
        &cli.out_dir,
        "rq3_kappa_ablation.csv",
        cli.allow_missing,
        write_rq3_ablation,
    )?;
    generate_if_exists(
        &cli.raw_dir,
        &cli.out_dir,
        "rq4_rollback.csv",
        cli.allow_missing,
        write_rq4_rollback,
    )?;
    generate_if_exists(
        &cli.raw_dir,
        &cli.out_dir,
        "termination.csv",
        cli.allow_missing,
        write_termination,
    )?;
    generate_if_exists(
        &cli.raw_dir,
        &cli.out_dir,
        "sensitivity.csv",
        cli.allow_missing,
        write_sensitivity,
    )?;
    generate_if_exists(
        &cli.raw_dir,
        &cli.out_dir,
        "scaling.csv",
        cli.allow_missing,
        write_scaling,
    )?;

    println!("Figure data generation complete.");
    Ok(())
}

fn generate_if_exists<F>(
    raw_dir: &Path,
    out_dir: &Path,
    name: &str,
    allow_missing: bool,
    f: F,
) -> Result<(), Box<dyn Error>>
where
    F: FnOnce(&Path, &Path) -> Result<(), Box<dyn Error>>,
{
    let path = raw_dir.join(name);
    if !path.exists() {
        if allow_missing {
            eprintln!("warning: missing {}, skipping", path.display());
            return Ok(());
        }
        return Err(format!("required input {} is missing", path.display()).into());
    }
    f(&path, out_dir)
}

fn load<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, Box<dyn Error>> {
    let mut reader = csv::Reader::from_path(path)?;
    let mut rows = Vec::new();
    for result in reader.deserialize() {
        rows.push(result?);
    }
    Ok(rows)
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn write_rq1_cdf(path: &Path, out_dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut vals: Vec<u64> = load::<Rq1Row>(path)?
        .into_iter()
        .filter(|r| r.strategy == "Sage" && r.cutover_latency_micros > 0)
        .map(|r| r.cutover_latency_micros)
        .collect();
    vals.sort_unstable();
    let mut out = String::from("latency_micros,cdf\n");
    for (idx, v) in vals.iter().enumerate() {
        out.push_str(&format!("{},{}\n", v, (idx + 1) as f64 / vals.len() as f64));
    }
    fs::write(out_dir.join("rq1_cutover_cdf.csv"), out)?;
    Ok(())
}

/// Observed migration downtime per strategy (mean + min/max, in milliseconds).
/// This is the RQ1 service-interruption contrast: a stop-the-world upgrade
/// halts for its snapshot/restart window, while zero-halt strategies stay at
/// normal block cadence. Plotted on a log scale in the paper because the gap
/// spans roughly two orders of magnitude.
fn write_rq1_downtime(path: &Path, out_dir: &Path) -> Result<(), Box<dyn Error>> {
    let rows = load::<Rq1Row>(path)?;
    let mut groups: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for r in rows {
        // Report in milliseconds for readability.
        groups
            .entry(r.strategy)
            .or_default()
            .push(r.downtime_micros as f64 / 1000.0);
    }
    let mut out =
        String::from("strategy,trials,mean_downtime_ms,min_downtime_ms,max_downtime_ms\n");
    // Stable, paper-facing order.
    for strategy in ["Sage", "StopTheWorld", "HardFork", "ReconfigOnly"] {
        if let Some(vals) = groups.get(strategy) {
            if vals.is_empty() {
                continue;
            }
            let n = vals.len();
            let mean = vals.iter().sum::<f64>() / n as f64;
            let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            out.push_str(&format!("{strategy},{n},{mean:.4},{min:.4},{max:.4}\n"));
        }
    }
    fs::write(out_dir.join("rq1_downtime.csv"), out)?;
    Ok(())
}

/// (observed safety violations, disjoint-quorum exposure flags) per group.
type Rq2Aggregate = (Vec<bool>, Vec<bool>);

fn write_rq2_fork_rate(path: &Path, out_dir: &Path) -> Result<(), Box<dyn Error>> {
    let rows = load::<Rq2Row>(path)?;
    // Track both the OBSERVED fork rate (safety_violation) and the
    // disjoint-quorum EXPOSURE rate (heights where a blind cutover left both
    // partition sides holding quorum). These are distinct: the serialized
    // simulator never observes a real conflict, but SAGE's CutCert is the only
    // strategy that eliminates the exposure window entirely.
    let mut groups: BTreeMap<(String, u32, u64), Rq2Aggregate> = BTreeMap::new();
    for r in rows {
        let entry = groups
            .entry((
                r.strategy,
                r.partition_split.unwrap_or(0),
                r.partition_duration_blocks.unwrap_or(0),
            ))
            .or_default();
        entry.0.push(r.safety_violation);
        entry.1.push(r.disjoint_quorum_windows > 0);
    }
    let mut out = String::from("strategy,split,duration,trials,fork_rate,exposure_rate\n");
    for ((strategy, split, duration), (forks, exposures)) in groups {
        let n = forks.len();
        let fork_count = forks.iter().filter(|v| **v).count();
        let exposure_count = exposures.iter().filter(|v| **v).count();
        out.push_str(&format!(
            "{strategy},{split},{duration},{},{:.4},{:.4}\n",
            n,
            fork_count as f64 / n as f64,
            exposure_count as f64 / n as f64
        ));
    }
    fs::write(out_dir.join("rq2_fork_rate.csv"), out)?;
    Ok(())
}

fn write_rq3_ablation(path: &Path, out_dir: &Path) -> Result<(), Box<dyn Error>> {
    let rows = load::<Rq3Row>(path)?;
    let mut groups: BTreeMap<u64, Vec<Rq3Row>> = BTreeMap::new();
    for r in rows {
        groups.entry(r.kappa).or_default().push(r);
    }
    let mut out =
        String::from("kappa,trials,unsafe_rate,migration_rate,mean_cutover_latency_micros\n");
    for (k, vals) in groups {
        let n = vals.len() as f64;
        let unsafe_rate = vals.iter().filter(|r| r.safety_violation).count() as f64 / n;
        let migration_rate = vals.iter().filter(|r| r.migration_success).count() as f64 / n;
        let lat: Vec<f64> = vals
            .iter()
            .map(|r| r.cutover_latency_micros as f64)
            .collect();
        out.push_str(&format!(
            "{k},{},{unsafe_rate:.4},{migration_rate:.4},{:.1}\n",
            vals.len(),
            mean(&lat)
        ));
    }
    fs::write(out_dir.join("rq3_ablation.csv"), out)?;
    Ok(())
}

fn write_rq4_rollback(path: &Path, out_dir: &Path) -> Result<(), Box<dyn Error>> {
    let rows = load::<Rq4Row>(path)?;
    let mut groups: BTreeMap<u64, Vec<Rq4Row>> = BTreeMap::new();
    for r in rows {
        groups.entry(r.abort_height).or_default().push(r);
    }
    let mut out = String::from(
        "abort_height,trials,rollback_rate,mean_rollback_latency_micros,rejection_rate\n",
    );
    for (h, vals) in groups {
        let n = vals.len() as f64;
        let rollback_rate = vals.iter().filter(|r| r.rollback_occurred).count() as f64 / n;
        let rejection_rate = vals
            .iter()
            .filter(|r| r.absolute_reversion_rejected)
            .count() as f64
            / n;
        let lat: Vec<f64> = vals
            .iter()
            .filter(|r| r.rollback_occurred)
            .map(|r| r.rollback_latency_micros as f64)
            .collect();
        out.push_str(&format!(
            "{h},{},{rollback_rate:.4},{:.1},{rejection_rate:.4}\n",
            vals.len(),
            mean(&lat)
        ));
    }
    fs::write(out_dir.join("rq4_rollback.csv"), out)?;
    Ok(())
}

fn write_termination(path: &Path, out_dir: &Path) -> Result<(), Box<dyn Error>> {
    let rows = load::<TerminationRow>(path)?;
    let mut groups: BTreeMap<(String, u64), Vec<TerminationRow>> = BTreeMap::new();
    for r in rows {
        groups
            .entry((r.strategy.clone(), r.partition_duration_blocks))
            .or_default()
            .push(r);
    }
    let mut out = String::from("strategy,duration,trials,stall_rate,safety_rate\n");
    for ((strategy, duration), vals) in groups {
        let n = vals.len() as f64;
        let stall = vals.iter().filter(|r| r.liveness_stall).count() as f64 / n;
        let safety = vals.iter().filter(|r| r.safety_violation).count() as f64 / n;
        out.push_str(&format!(
            "{strategy},{duration},{},{stall:.4},{safety:.4}\n",
            vals.len()
        ));
    }
    fs::write(out_dir.join("termination.csv"), out)?;
    Ok(())
}

fn write_sensitivity(path: &Path, out_dir: &Path) -> Result<(), Box<dyn Error>> {
    let rows = load::<SensitivityRow>(path)?;
    let mut groups: BTreeMap<u64, Vec<SensitivityRow>> = BTreeMap::new();
    for r in rows {
        groups.entry(r.mean_delay_micros).or_default().push(r);
    }
    let baseline = groups.keys().next().copied().unwrap_or(1);
    let baseline_blocks = groups
        .get(&baseline)
        .map(|v| {
            mean(
                &v.iter()
                    .map(|r| r.finalized_blocks as f64)
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or(1.0);
    let mut out = String::from(
        "mean_delay_micros,trials,mean_cutover_latency_micros,migration_rate,throughput_relative\n",
    );
    for (delay, vals) in groups {
        let lat: Vec<f64> = vals
            .iter()
            .map(|r| r.cutover_latency_micros as f64)
            .collect();
        let blocks = mean(
            &vals
                .iter()
                .map(|r| r.finalized_blocks as f64)
                .collect::<Vec<_>>(),
        );
        let migration_rate =
            vals.iter().filter(|r| r.migration_success).count() as f64 / vals.len() as f64;
        out.push_str(&format!(
            "{delay},{},{:.1},{migration_rate:.4},{:.4}\n",
            vals.len(),
            mean(&lat),
            blocks / baseline_blocks.max(1.0)
        ));
    }
    fs::write(out_dir.join("sensitivity.csv"), out)?;
    Ok(())
}

fn write_scaling(path: &Path, out_dir: &Path) -> Result<(), Box<dyn Error>> {
    let rows = load::<ScalingRow>(path)?;
    let mut groups: BTreeMap<u32, Vec<ScalingRow>> = BTreeMap::new();
    for r in rows {
        groups.entry(r.n).or_default().push(r);
    }
    let mut out =
        String::from("n,trials,mean_cutover_latency_micros,migration_rate,mean_finalized_blocks\n");
    for (n, vals) in groups {
        let lat: Vec<f64> = vals
            .iter()
            .map(|r| r.cutover_latency_micros as f64)
            .collect();
        let blocks: Vec<f64> = vals.iter().map(|r| r.finalized_blocks as f64).collect();
        let migration_rate =
            vals.iter().filter(|r| r.migration_success).count() as f64 / vals.len() as f64;
        out.push_str(&format!(
            "{n},{},{:.1},{migration_rate:.4},{:.1}\n",
            vals.len(),
            mean(&lat),
            mean(&blocks)
        ));
    }
    fs::write(out_dir.join("scaling.csv"), out)?;
    Ok(())
}
