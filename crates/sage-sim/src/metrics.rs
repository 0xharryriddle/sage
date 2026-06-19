use crate::StrategyKind;
use sage_core::{BlockHash, FinalizedBlock, Height, SimTime, ValidatorId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RunMetrics {
    pub finalized_blocks: u64,
    pub max_finalized_height: u64,
    pub safety_violation: bool,
    pub migration_success: bool,
    pub protocol_swap_success: bool,
    pub cutover_height: Option<u64>,
    pub cutover_latency_micros: Option<u64>,
    pub downtime_events: u64,
    /// Number of heights at which a non-CutCert cutover left BOTH partition
    /// sides simultaneously holding >= quorum voting power, i.e. a fork was
    /// *structurally possible*. This is an EXPOSURE measure, not an observed
    /// fork. Observed forks are reported separately via `safety_violation`.
    pub disjoint_quorum_windows: u64,
    /// Largest gap, in simulated microseconds, between the first-finalization
    /// timestamps of two consecutive heights. This is the OBSERVED service
    /// interruption: it is measured from when blocks actually finalize, never
    /// derived from the strategy. A stop-the-world upgrade halts finalization
    /// during its snapshot/restart window and therefore exhibits a large gap;
    /// a zero-halt strategy keeps this near normal block cadence.
    pub max_finalization_gap_micros: u64,
    /// Simulated microsecond timestamp at which the cutover height first
    /// finalized (the resume point), for provenance. `None` if never reached.
    pub cutover_finalize_micros: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct MetricsRecorder {
    strategy: StrategyKind,
    committed: BTreeMap<ValidatorId, Vec<FinalizedBlock>>,
    cutover_height: Option<Height>,
    cutover_latency_micros: Option<u64>,
    downtime_events: u64,
    disjoint_quorum_windows: u64,
    /// First simulated time at which each height finalized on any validator.
    first_finalize_time: BTreeMap<Height, SimTime>,
}

impl MetricsRecorder {
    pub fn new(strategy: StrategyKind) -> Self {
        Self {
            strategy,
            committed: BTreeMap::new(),
            cutover_height: None,
            cutover_latency_micros: None,
            downtime_events: 0,
            disjoint_quorum_windows: 0,
            first_finalize_time: BTreeMap::new(),
        }
    }

    pub fn observe_finalized(
        &mut self,
        validator: ValidatorId,
        block: FinalizedBlock,
        now: SimTime,
    ) {
        let height = block.block.header.height;
        self.first_finalize_time.entry(height).or_insert(now);
        self.committed.entry(validator).or_default().push(block);
    }

    pub fn observe_cutover(&mut self, height: Height) {
        self.cutover_height.get_or_insert(height);
    }

    pub fn observe_cutover_latency(&mut self, latency_micros: u64) {
        self.cutover_latency_micros.get_or_insert(latency_micros);
    }

    pub fn observe_downtime_event(&mut self) {
        self.downtime_events = self.downtime_events.saturating_add(1);
    }

    /// Record a height where both partition sides simultaneously held >= quorum
    /// under a non-CutCert cutover (fork *exposure*, not an observed fork).
    pub fn observe_disjoint_quorum_window(&mut self) {
        self.disjoint_quorum_windows = self.disjoint_quorum_windows.saturating_add(1);
    }

    pub fn finish(&self) -> RunMetrics {
        let finalized_blocks = self.committed.values().map(|v| v.len() as u64).sum();
        let max_finalized_height = self
            .committed
            .values()
            .flat_map(|blocks| blocks.iter().map(|b| b.block.header.height.get()))
            .max()
            .unwrap_or_default();
        // Observed service interruption: the largest gap between the first
        // finalization times of consecutive heights. Measured purely from
        // recorded timestamps, so a strategy that genuinely halts shows a
        // large gap and a zero-halt strategy does not.
        let mut max_gap = 0u64;
        let mut prev: Option<SimTime> = None;
        for time in self.first_finalize_time.values() {
            if let Some(p) = prev {
                max_gap = max_gap.max(time.0.saturating_sub(p.0));
            }
            prev = Some(*time);
        }
        let cutover_finalize_micros = self
            .cutover_height
            .and_then(|h| self.first_finalize_time.get(&h))
            .map(|t| t.0);
        // Safety violation is an OBSERVED fact: two distinct finalized block
        // hashes at the same height across validators. It is never inferred from
        // a heuristic — the experiment must surface a real conflict.
        RunMetrics {
            finalized_blocks,
            max_finalized_height,
            safety_violation: self.has_conflict(),
            migration_success: self.cutover_height.is_some()
                || !self.strategy.uses_shadow_validation(),
            protocol_swap_success: self.strategy.completes_protocol_swap(),
            cutover_height: self.cutover_height.map(Height::get),
            cutover_latency_micros: self.cutover_latency_micros,
            downtime_events: self.downtime_events,
            disjoint_quorum_windows: self.disjoint_quorum_windows,
            max_finalization_gap_micros: max_gap,
            cutover_finalize_micros,
        }
    }

    fn has_conflict(&self) -> bool {
        let mut by_height: BTreeMap<Height, BTreeSet<BlockHash>> = BTreeMap::new();
        for blocks in self.committed.values() {
            for finalized in blocks {
                by_height
                    .entry(finalized.block.header.height)
                    .or_default()
                    .insert(finalized.block.hash());
            }
        }
        by_height.values().any(|hashes| hashes.len() > 1)
    }
}
