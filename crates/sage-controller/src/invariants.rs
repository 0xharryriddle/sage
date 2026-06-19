//! Executable invariant checks over compact system snapshots.
use sage_core::{BlockHash, EngineId, FinalityTier, Height, ValidatorId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub validators: Vec<ValidatorSnapshot>,
    pub cutovers: Vec<CutoverSummary>,
    pub rollbacks: Vec<RollbackSummary>,
    pub shadow_events: Vec<ShadowSummary>,
}

impl SystemSnapshot {
    pub fn correct_validators(&self) -> impl Iterator<Item = &ValidatorSnapshot> {
        self.validators.iter().filter(|v| v.correct)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorSnapshot {
    pub validator: ValidatorId,
    pub correct: bool,
    pub authoritative_engine: EngineId,
    pub committed: Vec<CommittedBlockSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommittedBlockSummary {
    pub height: Height,
    pub block_hash: BlockHash,
    pub engine: EngineId,
    pub finality_tier: FinalityTier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutoverSummary {
    pub height: Height,
    pub engine: EngineId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollbackSummary {
    pub at_height: Height,
    pub reverted: Vec<CommittedBlockSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowSummary {
    pub validator: ValidatorId,
    pub height: Height,
    pub finalized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvariantReport {
    pub checks: Vec<InvariantCheck>,
}

impl InvariantReport {
    pub fn all_passed(&self) -> bool {
        self.checks
            .iter()
            .all(|check| check.status == InvariantStatus::Pass)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvariantCheck {
    pub name: String,
    pub status: InvariantStatus,
    pub message: String,
}

impl InvariantCheck {
    fn pass(name: &str, message: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status: InvariantStatus::Pass,
            message: message.into(),
        }
    }

    fn fail(name: &str, message: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status: InvariantStatus::Fail,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvariantStatus {
    Pass,
    Fail,
}

pub fn check_all(snapshot: &SystemSnapshot) -> InvariantReport {
    InvariantReport {
        checks: vec![
            check_single_finalizer(snapshot),
            check_no_conflicting_commits(snapshot),
            check_cutcert_unique(snapshot),
            check_no_absolute_reversion(snapshot),
            check_shadow_never_finalizes(snapshot),
        ],
    }
}

pub fn check_single_finalizer(snapshot: &SystemSnapshot) -> InvariantCheck {
    let engines: BTreeSet<_> = snapshot
        .correct_validators()
        .map(|v| v.authoritative_engine)
        .collect();
    if engines.len() <= 1 {
        InvariantCheck::pass("single_finalizer", "correct validators agree on finalizer")
    } else {
        InvariantCheck::fail(
            "single_finalizer",
            format!(
                "{} authoritative engines among correct validators",
                engines.len()
            ),
        )
    }
}

pub fn check_no_conflicting_commits(snapshot: &SystemSnapshot) -> InvariantCheck {
    let mut by_height: BTreeMap<Height, BTreeSet<BlockHash>> = BTreeMap::new();
    for validator in snapshot.correct_validators() {
        for block in &validator.committed {
            by_height
                .entry(block.height)
                .or_default()
                .insert(block.block_hash);
        }
    }
    for (height, hashes) in by_height {
        if hashes.len() > 1 {
            return InvariantCheck::fail(
                "no_conflicting_commits",
                format!("{} conflicting hashes at height {}", hashes.len(), height),
            );
        }
    }
    InvariantCheck::pass(
        "no_conflicting_commits",
        "no conflicting committed block hashes",
    )
}

pub fn check_cutcert_unique(snapshot: &SystemSnapshot) -> InvariantCheck {
    let mut by_height: BTreeMap<Height, BTreeSet<EngineId>> = BTreeMap::new();
    for cutover in &snapshot.cutovers {
        by_height
            .entry(cutover.height)
            .or_default()
            .insert(cutover.engine);
    }
    for (height, engines) in by_height {
        if engines.len() > 1 {
            return InvariantCheck::fail(
                "cutcert_unique",
                format!("{} cutover engines at height {}", engines.len(), height),
            );
        }
    }
    InvariantCheck::pass(
        "cutcert_unique",
        "cutover certificates are unique by height",
    )
}

pub fn check_no_absolute_reversion(snapshot: &SystemSnapshot) -> InvariantCheck {
    for rollback in &snapshot.rollbacks {
        if rollback
            .reverted
            .iter()
            .any(|block| block.finality_tier == FinalityTier::Absolute)
        {
            return InvariantCheck::fail(
                "no_absolute_reversion",
                format!("rollback at {} reverts absolute block", rollback.at_height),
            );
        }
    }
    InvariantCheck::pass(
        "no_absolute_reversion",
        "no rollback reverts absolute-final blocks",
    )
}

pub fn check_shadow_never_finalizes(snapshot: &SystemSnapshot) -> InvariantCheck {
    if let Some(event) = snapshot.shadow_events.iter().find(|event| event.finalized) {
        return InvariantCheck::fail(
            "shadow_never_finalizes",
            format!("shadow finalized at height {}", event.height),
        );
    }
    InvariantCheck::pass(
        "shadow_never_finalizes",
        "shadow validations never finalize",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_core::{EngineGeneration, EngineKind, Hash32};

    fn engine(kind: EngineKind) -> EngineId {
        EngineId::new(kind, EngineGeneration::new(1))
    }

    fn block(height: u64, byte: u8) -> CommittedBlockSummary {
        CommittedBlockSummary {
            height: Height::new(height),
            block_hash: Hash32::new([byte; 32]),
            engine: engine(EngineKind::Poa),
            finality_tier: FinalityTier::Absolute,
        }
    }

    fn snapshot_with(
        blocks_a: Vec<CommittedBlockSummary>,
        blocks_b: Vec<CommittedBlockSummary>,
    ) -> SystemSnapshot {
        SystemSnapshot {
            validators: vec![
                ValidatorSnapshot {
                    validator: ValidatorId::new(0),
                    correct: true,
                    authoritative_engine: engine(EngineKind::Poa),
                    committed: blocks_a,
                },
                ValidatorSnapshot {
                    validator: ValidatorId::new(1),
                    correct: true,
                    authoritative_engine: engine(EngineKind::Poa),
                    committed: blocks_b,
                },
            ],
            cutovers: vec![],
            rollbacks: vec![],
            shadow_events: vec![],
        }
    }

    #[test]
    fn identical_logs_pass_conflict_check() {
        let snapshot = snapshot_with(vec![block(1, 1)], vec![block(1, 1)]);
        assert_eq!(
            check_no_conflicting_commits(&snapshot).status,
            InvariantStatus::Pass
        );
    }

    #[test]
    fn conflicting_logs_fail_conflict_check() {
        let snapshot = snapshot_with(vec![block(1, 1)], vec![block(1, 2)]);
        assert_eq!(
            check_no_conflicting_commits(&snapshot).status,
            InvariantStatus::Fail
        );
    }

    #[test]
    fn shadow_finalization_fails() {
        let mut snapshot = snapshot_with(vec![], vec![]);
        snapshot.shadow_events.push(ShadowSummary {
            validator: ValidatorId::new(0),
            height: Height::new(1),
            finalized: true,
        });
        assert_eq!(
            check_shadow_never_finalizes(&snapshot).status,
            InvariantStatus::Fail
        );
    }
}
