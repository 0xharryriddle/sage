use crate::{SimError, SimResult, StrategyKind};
use sage_controller::Schedule;
use sage_core::Height;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimConfig {
    pub simulation: SimulationConfig,
    pub validators: ValidatorsConfig,
    pub workload: WorkloadConfig,
    pub strategy: StrategyKind,
    pub migration: Schedule,
    pub adversary: AdversaryConfig,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            simulation: SimulationConfig::default(),
            validators: ValidatorsConfig::default(),
            workload: WorkloadConfig::default(),
            strategy: StrategyKind::default(),
            migration: Schedule {
                h_d: Height::new(20),
                h_c: Height::new(30),
                h_r: Height::new(40),
                kappa: 1,
                tau_blocks: 2,
            },
            adversary: AdversaryConfig::default(),
        }
    }
}

impl SimConfig {
    pub fn load(path: impl AsRef<Path>) -> SimResult<Self> {
        let text = fs::read_to_string(path)?;
        let cfg: Self = toml::from_str(&text)?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> SimResult<()> {
        if self.simulation.max_height == 0 {
            return Err(SimError::Config("max_height must be > 0".to_string()));
        }
        if self.validators.n == 0 {
            return Err(SimError::Config("validator count must be > 0".to_string()));
        }
        if self.workload.txs_per_block == 0 {
            return Err(SimError::Config("txs_per_block must be > 0".to_string()));
        }
        self.migration
            .validate()
            .map_err(|err| SimError::Config(err.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationConfig {
    pub seed: u64,
    pub max_height: u64,
    pub mean_delay_micros: u64,
    pub max_events: u64,
    /// Simulated-time halt (microseconds) that a stop-the-world upgrade incurs
    /// at its restart height while it snapshots state and initializes the
    /// target engine. Modeled as a single service interruption: the chain
    /// stops finalizing for this window, then resumes. Only `StopTheWorld`
    /// uses it; zero-halt strategies ignore it. Defaults to 0 so existing
    /// configs are unchanged; RQ1/baseline configs set it explicitly.
    #[serde(default)]
    pub stw_downtime_micros: u64,
}
impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            seed: 1,
            max_height: 4,
            mean_delay_micros: 40_000,
            max_events: 100_000,
            stw_downtime_micros: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorsConfig {
    pub n: u32,
    pub f: u64,
}
impl Default for ValidatorsConfig {
    fn default() -> Self {
        Self { n: 4, f: 1 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadConfig {
    pub state_accounts: u64,
    pub initial_balance: u64,
    pub txs_per_block: usize,
}
impl Default for WorkloadConfig {
    fn default() -> Self {
        Self {
            state_accounts: 16,
            initial_balance: 1_000,
            txs_per_block: 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdversaryConfig {
    pub partition_enabled: bool,
    pub partition_start_height: u64,
    pub partition_duration_blocks: u64,
    pub partition_split: u32,
    pub byzantine_count: u32,
}
impl Default for AdversaryConfig {
    fn default() -> Self {
        Self {
            partition_enabled: false,
            partition_start_height: 2,
            partition_duration_blocks: 2,
            partition_split: 2,
            byzantine_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_config_is_valid() {
        assert!(SimConfig::default().validate().is_ok());
    }
}
