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
    /// Cox switching on its ACTUAL safety mechanism: a StableCheckpoint
    /// certified by the target engine's BFT quorum (2f+1), rather than "no gate"
    /// (CoxStyle) or SAGE's n-f gate. This models Cox's real quorum threshold on
    /// our engine interface. It is IDENTICAL to the SAGE code path except the
    /// cutover-quorum threshold is 2f+1 (Cox's checkpoint quorum) instead of n-f
    /// (SAGE's gate) — the cleanest possible isolation of the contribution. It
    /// is NOT a line-for-line Cox port: it omits Cox's epoch-mismatch catch-up
    /// sub-protocol (a liveness/completeness feature, not safety-relevant to the
    /// boundary fork differential). With n=6, f=1 a 2f+1=3 checkpoint quorum is
    /// reachable by EACH side of a 3/3 partition (2 disjoint 3-quorums fit in 6
    /// once n>3f+1), so Cox's own gate still forks at the heterogeneous
    /// boundary; SAGE's n-f=5 is unreachable by a side of 3. OBSERVED, never
    /// derived from the flag.
    CoxFaithful,
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

    /// Synthetic transactions minted per block for the throughput measurement.
    /// These are deterministic and HEIGHT-derived (not proposer-derived), so
    /// every correct proposer at a given height mints a byte-identical block —
    /// keeping the no-fault path fork-free while giving a real, non-zero
    /// committed-transaction count to measure TPS from. 0 (default) preserves
    /// the original empty-block behavior for the fork/partition experiments,
    /// where proposer-dependent coinbase (not this) is what must vary.
    pub workload_txs_per_block: u64,
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
            workload_txs_per_block: 0,
        }
    }

    /// Builder-style override for the migration strategy.
    pub fn with_strategy(mut self, strategy: NodeStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Builder-style override for the synthetic per-block workload used by the
    /// throughput measurement.
    pub fn with_workload_txs_per_block(mut self, txs: u64) -> Self {
        self.workload_txs_per_block = txs;
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
