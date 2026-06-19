//! Observed-fork detector for the multi-process testbed.
//!
//! This is the artifact the publishability review identified as load-bearing:
//! a detector that reports an *observed* cross-validator fork (two distinct
//! finalized block hashes at the same height) rather than a proxy. Critically,
//! it ships with a broken-control test that deliberately constructs a fork to
//! prove the detector actually fires — a detector that never fires proves
//! nothing.
//!
//! The detector is a pure function over each validator's committed log, so it
//! works identically for the in-process `LocalRuntime` and a future
//! multi-process testbed that collects logs over the network.

use sage_core::{BlockHash, FinalizedBlock, Height, ValidatorId};
use std::collections::{BTreeMap, BTreeSet};

/// One observed conflict: a single height at which validators committed two or
/// more distinct block hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkEvidence {
    pub height: Height,
    /// The distinct block hashes observed at this height, with the set of
    /// validators that committed each one.
    pub conflicting: BTreeMap<BlockHash, BTreeSet<ValidatorId>>,
}

/// Result of scanning all validators' committed logs for cross-boundary forks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForkReport {
    pub forks: Vec<ForkEvidence>,
}

impl ForkReport {
    /// True iff at least one height carries two distinct committed hashes.
    pub fn fork_observed(&self) -> bool {
        !self.forks.is_empty()
    }

    /// Number of distinct heights at which a fork was observed.
    pub fn fork_count(&self) -> usize {
        self.forks.len()
    }
}

/// Scan per-validator committed logs and report any height at which two
/// validators committed different block hashes. This is the executable
/// definition of a consensus safety violation across the migration boundary.
pub fn detect_forks(
    committed_by_validator: &BTreeMap<ValidatorId, Vec<FinalizedBlock>>,
) -> ForkReport {
    // height -> (block hash -> validators that committed it)
    let mut by_height: BTreeMap<Height, BTreeMap<BlockHash, BTreeSet<ValidatorId>>> =
        BTreeMap::new();
    for (validator, blocks) in committed_by_validator {
        for fb in blocks {
            let height = fb.block.header.height;
            let hash = fb.block.hash();
            by_height
                .entry(height)
                .or_default()
                .entry(hash)
                .or_default()
                .insert(*validator);
        }
    }

    let mut forks = Vec::new();
    for (height, hashes) in by_height {
        if hashes.len() > 1 {
            forks.push(ForkEvidence {
                height,
                conflicting: hashes,
            });
        }
    }
    ForkReport { forks }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_core::{
        Block, BlockHeader, ChainId, ConfigId, EngineGeneration, EngineId, EngineKind, Epoch,
        FinalityTier, Hash32, StateRoot, Transaction,
    };

    fn block_at(height: u64, state_root: u8, txs: Vec<Transaction>) -> FinalizedBlock {
        let block = Block {
            header: BlockHeader {
                chain_id: ChainId::new("sage"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                height: Height::new(height),
                parent_hash: Hash32::ZERO,
                state_root: StateRoot::new([state_root; 32]),
                engine_id: EngineId::new(EngineKind::Poa, EngineGeneration::new(1)),
                finality_tier: FinalityTier::Absolute,
                manifest_hash: None,
            },
            txs,
        };
        let hash = block.hash();
        FinalizedBlock {
            block,
            certificate_hash: hash,
        }
    }

    /// Honest path: all validators committed the SAME block at each height.
    /// The detector must report NO fork.
    #[test]
    fn no_fork_when_logs_agree() {
        let mut logs: BTreeMap<ValidatorId, Vec<FinalizedBlock>> = BTreeMap::new();
        for v in 0..4u32 {
            logs.insert(
                ValidatorId::new(v),
                vec![block_at(1, 0xAA, vec![]), block_at(2, 0xBB, vec![])],
            );
        }
        let report = detect_forks(&logs);
        assert!(!report.fork_observed(), "agreeing logs must not fork");
        assert_eq!(report.fork_count(), 0);
    }

    /// BROKEN CONTROL: two partition sides commit DIFFERENT blocks at height 2.
    /// This deliberately injects a fork to prove the detector fires. Without
    /// this test, a never-firing detector would look identical to a correct one.
    #[test]
    fn detector_fires_on_injected_fork() {
        let mut logs: BTreeMap<ValidatorId, Vec<FinalizedBlock>> = BTreeMap::new();
        // Side A (validators 0,1) commit one block at height 2.
        let side_a = block_at(2, 0x11, vec![]);
        // Side B (validators 2,3) commit a DIFFERENT block at height 2
        // (different state root -> different hash), as a partition fork would.
        let side_b = block_at(2, 0x22, vec![]);
        // Sanity: the two blocks really are distinct.
        assert_ne!(side_a.block.hash(), side_b.block.hash());

        let h1 = block_at(1, 0xAA, vec![]);
        logs.insert(ValidatorId::new(0), vec![h1.clone(), side_a.clone()]);
        logs.insert(ValidatorId::new(1), vec![h1.clone(), side_a.clone()]);
        logs.insert(ValidatorId::new(2), vec![h1.clone(), side_b.clone()]);
        logs.insert(ValidatorId::new(3), vec![h1, side_b]);

        let report = detect_forks(&logs);
        assert!(
            report.fork_observed(),
            "detector MUST fire on an injected cross-side fork"
        );
        assert_eq!(report.fork_count(), 1, "exactly one forked height");
        let ev = &report.forks[0];
        assert_eq!(ev.height, Height::new(2));
        assert_eq!(ev.conflicting.len(), 2, "two distinct hashes at height 2");
        // Each conflicting hash is backed by its two-validator side.
        for signers in ev.conflicting.values() {
            assert_eq!(signers.len(), 2);
        }
    }

    /// Height 1 agrees, height 2 forks: detector isolates only the bad height.
    #[test]
    fn detector_isolates_forked_height() {
        let mut logs: BTreeMap<ValidatorId, Vec<FinalizedBlock>> = BTreeMap::new();
        let h1 = block_at(1, 0xAA, vec![]);
        logs.insert(
            ValidatorId::new(0),
            vec![h1.clone(), block_at(2, 0x11, vec![])],
        );
        logs.insert(ValidatorId::new(1), vec![h1, block_at(2, 0x22, vec![])]);
        let report = detect_forks(&logs);
        assert_eq!(report.fork_count(), 1);
        assert_eq!(report.forks[0].height, Height::new(2));
    }
}
