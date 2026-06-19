//! Shared CLI, config loading, seed parsing, CSV output, and metadata helpers.
use crate::error::{ExperimentError, ExperimentResult};
use sage_sim::SimConfig;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Standard CLI shared across experiment binaries.
#[derive(clap::Parser)]
pub struct ExperimentCli {
    /// Path to TOML config file
    #[arg(long, default_value = "config/default.toml")]
    pub config: std::path::PathBuf,

    /// Output directory for raw CSV results
    #[arg(long, default_value = "results/raw")]
    pub out_dir: std::path::PathBuf,

    /// Comma-separated seed list (e.g. --seeds 0,1,2,3,4)
    /// Overrides the config simulation.seed for each trial.
    #[arg(long, value_delimiter = ',')]
    pub seeds: Option<Vec<u64>>,

    /// Number of trials when --seeds is not provided
    #[arg(long, default_value = "5")]
    pub trials: u64,
}

impl ExperimentCli {
    /// Resolve seed list from either --seeds flag or generate 0..trials.
    pub fn seed_list(&self) -> Vec<u64> {
        match &self.seeds {
            Some(v) if !v.is_empty() => v.clone(),
            _ => (0..self.trials).collect(),
        }
    }
}

/// Load a SimConfig from a TOML file.
pub fn load_config(path: impl AsRef<Path>) -> ExperimentResult<SimConfig> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(ExperimentError::MissingConfig {
            path: path.to_path_buf(),
        });
    }
    SimConfig::load(path).map_err(|e| ExperimentError::Simulation(e.to_string()))
}

/// Write a slice of serializable rows to a CSV file.
pub fn write_csv<T: Serialize>(path: impl AsRef<Path>, rows: &[T]) -> ExperimentResult<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| ExperimentError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let mut wtr = csv::Writer::from_path(path).map_err(|e| ExperimentError::Csv {
        path: path.to_path_buf(),
        source: e,
    })?;
    for row in rows {
        wtr.serialize(row).map_err(|e| ExperimentError::Csv {
            path: path.to_path_buf(),
            source: e,
        })?;
    }
    wtr.flush().map_err(|e| ExperimentError::Csv {
        path: path.to_path_buf(),
        source: csv::Error::from(e),
    })?;
    Ok(())
}

/// Write metadata JSON for an experiment run. Records config hash,
/// seed, timestamp, and run status for reproducibility.
pub fn stable_config_hash(config_toml: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(config_toml.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn command_version(command: &str, arg: &str) -> Option<String> {
    let output = Command::new(command).arg(arg).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout.lines().next().map(str::to_string)
}

pub fn write_metadata(
    out_dir: impl AsRef<Path>,
    experiment_name: &str,
    config_path: impl AsRef<Path>,
    config_toml: &str,
    seed: u64,
    run_status: &str,
    metrics: &sage_sim::RunMetrics,
) -> ExperimentResult<()> {
    use std::collections::BTreeMap;
    let out_dir = out_dir.as_ref();
    let config_path = config_path.as_ref();
    let meta_dir = out_dir.join("metadata");
    fs::create_dir_all(&meta_dir).map_err(|e| ExperimentError::Io {
        path: meta_dir.clone(),
        source: e,
    })?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let config_hash = stable_config_hash(config_toml);

    let meta_filename = format!("{}_{:016x}.json", experiment_name, seed);
    let meta: BTreeMap<String, serde_json::Value> = {
        let mut m = BTreeMap::new();
        m.insert("experiment".into(), experiment_name.into());
        m.insert("seed".into(), seed.into());
        m.insert(
            "config_path".into(),
            config_path.to_string_lossy().to_string().into(),
        );
        m.insert("config_hash_alg".into(), "sha256".into());
        m.insert("config_hash".into(), config_hash.into());
        m.insert("artifact_version".into(), env!("CARGO_PKG_VERSION").into());
        if let Some(version) = command_version("rustc", "--version") {
            m.insert("rustc_version".into(), version.into());
        }
        if let Some(version) = command_version("cargo", "--version") {
            m.insert("cargo_version".into(), version.into());
        }
        m.insert("timestamp_unix".into(), now.into());
        m.insert("run_status".into(), run_status.into());
        if let Ok(json) = serde_json::to_value(metrics) {
            m.insert("metrics".into(), json);
        }
        m
    };

    let out_path = meta_dir.join(&meta_filename);
    let json = serde_json::to_string_pretty(&meta).map_err(|e| ExperimentError::Io {
        path: out_path.clone(),
        source: std::io::Error::other(e),
    })?;
    fs::write(&out_path, json).map_err(|e| ExperimentError::Io {
        path: out_path,
        source: e,
    })?;
    Ok(())
}

/// Run a single simulation and return metrics (or map errors to None).
pub fn run_single(
    cfg: &SimConfig,
    strategy: sage_sim::StrategyKind,
) -> Option<sage_sim::RunMetrics> {
    let mut cfg = cfg.clone();
    cfg.strategy = strategy;
    match sage_sim::Simulation::new(cfg) {
        Ok(mut sim) => match sim.run() {
            Ok(metrics) => Some(metrics),
            Err(e) => {
                eprintln!("sim error: {:?}", e);
                None
            }
        },
        Err(e) => {
            eprintln!("create error: {:?}", e);
            None
        }
    }
}
