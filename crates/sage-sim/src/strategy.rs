use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StrategyKind {
    StopTheWorld,
    HardFork,
    ReconfigOnly,
    #[default]
    Sage,
    SageBlind,
    /// Cox-style live homogeneous switch (modeled on our engine interface, NOT
    /// a faithful Cox reimplementation). Models the closest live-switch SOTA
    /// (Cox, Blockchain: Research and Applications 2026): a zero-downtime switch
    /// that forms a quorum-certified boundary checkpoint analogous to Cox's
    /// StableCheckpoint, with forward catch-up recovery. It CREDITS Cox's
    /// strengths (zero halt, quorum checkpoint) and isolates exactly what Cox's
    /// HOMOGENEOUS design does not provide for a HETEROGENEOUS PoA->BFT crossing:
    /// no shadow-anchored target bootstrap validation, no n-f dual-run
    /// matching-root cutover predicate, and no verifiable bounded rollback.
    /// Cox's checkpoint certifies legacy-epoch termination among homogeneous
    /// validators; it does not certify that the heterogeneous target engine can
    /// reconstruct identical state. The fork/safety consequence of that gap is
    /// OBSERVED, never derived from this flag.
    CoxStyle,
}

impl StrategyKind {
    pub fn completes_protocol_swap(self) -> bool {
        !matches!(self, Self::ReconfigOnly)
    }

    pub fn requires_cutcert(self) -> bool {
        matches!(self, Self::Sage)
    }

    pub fn uses_shadow_validation(self) -> bool {
        matches!(self, Self::Sage | Self::SageBlind)
    }

    pub fn has_planned_downtime(self) -> bool {
        matches!(self, Self::StopTheWorld)
    }

    /// Whether this strategy switches engines on a live, zero-downtime schedule
    /// at the cutover height (no halt). True for the Cox-style live switch and
    /// the blind/hard-fork cutovers; false for StopTheWorld (halts) and
    /// ReconfigOnly (never swaps the engine). SAGE also switches live but gates
    /// the switch on the n-f cutover quorum, so it is handled via the controller.
    pub fn live_switch_at_cutover(self) -> bool {
        matches!(self, Self::HardFork | Self::SageBlind | Self::CoxStyle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconfig_only_does_not_complete_protocol_swap() {
        assert!(!StrategyKind::ReconfigOnly.completes_protocol_swap());
    }

    #[test]
    fn only_sage_requires_cutcert() {
        assert!(StrategyKind::Sage.requires_cutcert());
        assert!(!StrategyKind::SageBlind.requires_cutcert());
    }
}
