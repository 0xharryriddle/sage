//! Configuration for the local multi-validator runtime.
use sage_consensus::ValidatorSet;
use sage_controller::Schedule;
use sage_core::{ChainId, ConfigId, Epoch, Height, SimTime, ValidatorId};

/// Migration strategy the runtime applies. Mirrors `sage-sim`'s `StrategyKind`
/// so the local testbed can exercise the same baselines (notably HardFork,
/// which can produce an observed fork under partition, versus Sage which cannot).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeStrategy {
    #[default]
    Sage,
    HardFork,
    SageBlind,
    StopTheWorld,
    ReconfigOnly,
    /// Cox-style live homogeneous switch (modeled, not a faithful Cox reimpl).
    /// Switches engines at the cutover height with a boundary checkpoint and no
    /// halt, but WITHOUT SAGE's n-f dual-run cutover quorum gate and WITHOUT
    /// verifiable rollback. In the multi-process testbed this isolates the
    /// safety consequence of crossing the heterogeneous boundary without the
    /// n-f gate — OBSERVED, never derived from the flag.
    CoxStyle,
}

/// Configuration for a local multi-validator runtime.
#[derive(Debug, Clone)]
pub struct NodeConfig {
    // ── Validator parameters ──
    /// Number of validators.
    pub n: u32,
    /// Number of faults tolerated.
    pub f: u32,
    /// Chain identifier.
    pub chain_id: ChainId,
    /// Configuration identifier.
    pub config_id: ConfigId,
    /// Epoch number.
    pub epoch: Epoch,
    /// Migration schedule.
    pub migration: Schedule,
    /// Maximum block height to finalize before stopping.
    pub max_height: Height,
    /// Maximum wall-clock runtime in seconds (0 = unlimited).
    pub max_runtime_secs: u64,
    /// Seed for deterministic block content generation.
    pub workload_seed: u64,

    // ── Pacemaker timing ──
    /// How often (in ms) each validator checks for timeout advancement.
    pub pacemaker_tick_ms: u64,
    /// HotStuff view timeout in ms (how long before emitting a timeout message).
    pub view_timeout_ms: u64,

    // ── Proposal interval ──
    /// How often (in ms) the leader proposes a new block.
    pub proposal_interval_ms: u64,

    // ── Network parameters ──
    /// Simulated mean network delay in microseconds.
    pub mean_delay_micros: u64,

    /// Migration strategy applied by every validator.
    pub strategy: NodeStrategy,
}

impl NodeConfig {
    /// Create a default config for a small local testnet.
    pub fn local_testnet(n: u32, f: u32, schedule: Schedule, max_height: Height) -> Self {
        Self {
            n,
            f,
            chain_id: ChainId::new("sage-local"),
            config_id: ConfigId::new(1),
            epoch: Epoch::new(1),
            migration: schedule,
            max_height,
            max_runtime_secs: 30,
            workload_seed: 42,
            pacemaker_tick_ms: 50,
            view_timeout_ms: 200,
            proposal_interval_ms: 50,
            mean_delay_micros: 5_000,
            strategy: NodeStrategy::Sage,
        }
    }

    /// Builder-style override for the migration strategy.
    pub fn with_strategy(mut self, strategy: NodeStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Return the validator IDs [0, 1, …, n-1].
    pub fn validator_ids(&self) -> Vec<ValidatorId> {
        (0..self.n).map(ValidatorId::new).collect()
    }

    /// Build an equal-power validator set.
    pub fn validator_set(&self) -> ValidatorSet {
        ValidatorSet::equal_power(self.config_id, self.epoch, self.n)
    }

    /// A [`SimTime`] constant derived from mean delay.
    pub fn delay(&self) -> SimTime {
        SimTime(self.mean_delay_micros)
    }
}
