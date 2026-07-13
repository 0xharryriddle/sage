use crate::{ControllerError, ControllerResult, Schedule};
use sage_core::crypto::{encode_bytes, encode_u64, hash_domain, HashDomain};
use sage_core::{BlockHash, ChainId, FinalizedBlock, Hash32, Height, StateRoot, Transaction};
use sage_manifest::Certificate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayBlockContext {
    pub chain_id: ChainId,
    pub height: Height,
    pub timestamp_micros: u64,
    pub beneficiary: Hash32,
    pub base_fee: u64,
    pub randomness: Hash32,
    pub oracle_snapshot_root: Option<Hash32>,
}

impl ReplayBlockContext {
    fn encode_for_root(&self, out: &mut Vec<u8>) {
        encode_bytes(out, self.chain_id.as_str().as_bytes());
        encode_u64(out, self.height.get());
        encode_u64(out, self.timestamp_micros);
        encode_bytes(out, self.beneficiary.as_bytes());
        encode_u64(out, self.base_fee);
        encode_bytes(out, self.randomness.as_bytes());
        match self.oracle_snapshot_root {
            Some(root) => {
                out.push(1);
                encode_bytes(out, root.as_bytes());
            }
            None => out.push(0),
        }
    }

    pub fn encoded_len_for_root(&self) -> usize {
        let mut out = Vec::new();
        self.encode_for_root(&mut out);
        out.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReplayContext {
    blocks: Vec<ReplayBlockContext>,
}

impl ReplayContext {
    pub fn new(blocks: Vec<ReplayBlockContext>) -> Self {
        Self { blocks }
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn blocks(&self) -> &[ReplayBlockContext] {
        &self.blocks
    }

    pub fn root(&self) -> Hash32 {
        let mut out = Vec::new();
        encode_u64(&mut out, self.blocks.len() as u64);
        for block in &self.blocks {
            block.encode_for_root(&mut out);
        }
        hash_domain(HashDomain::ReplayContextV1, &out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollbackAnchor {
    pub boundary_height: Height,
    pub boundary_root: StateRoot,
    pub boundary_hash: BlockHash,
    pub legacy_certificate: Certificate,
    pub replay_context_root: Option<Hash32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum AbortTrigger {
    ProgressTimeout,
    AbortCert(Certificate),
    ManifestMismatch,
    OperatorFailClosed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollbackOutcome {
    pub restored_height: Height,
    pub restored_root: StateRoot,
    pub replay_transactions: Vec<Transaction>,
    pub replay_context: ReplayContext,
    pub discarded_blocks: usize,
}

#[derive(Debug, Clone, Default)]
pub struct RollbackManager {
    anchor: Option<RollbackAnchor>,
    provisional: Vec<FinalizedBlock>,
    replay_context: ReplayContext,
}

impl RollbackManager {
    pub fn install_anchor(&mut self, anchor: RollbackAnchor) {
        self.anchor = Some(anchor);
    }
    pub fn record_provisional(&mut self, block: FinalizedBlock) {
        self.provisional.push(block);
    }

    pub fn record_replay_context(&mut self, context: ReplayBlockContext) {
        self.replay_context.blocks.push(context);
    }

    pub fn rollback(
        &mut self,
        _trigger: AbortTrigger,
        at: Height,
        schedule: &Schedule,
    ) -> ControllerResult<RollbackOutcome> {
        if !schedule.rollback_open(at) {
            return Err(ControllerError::RollbackDeadlinePassed { height: at });
        }
        let anchor = self
            .anchor
            .clone()
            .ok_or(ControllerError::MissingRollbackAnchor)?;
        let replay_transactions = self
            .provisional
            .iter()
            .flat_map(|b| b.block.txs.clone())
            .collect();
        // Validate borrowed recovery state first. A rejected rollback must retain
        // all evidence so an operator can inspect or retry it without changing
        // the failure mode merely by calling this method once.
        validate_replay_context(&anchor, &self.provisional, &self.replay_context)?;
        let actual_root = self.replay_context.root();
        match anchor.replay_context_root {
            Some(expected_root) if actual_root != expected_root => {
                return Err(ControllerError::ReplayContextRootMismatch {
                    expected: expected_root,
                    actual: actual_root,
                });
            }
            Some(_) => {}
            None if !self.replay_context.is_empty() => {
                return Err(ControllerError::MissingReplayContextRoot {
                    blocks: self.replay_context.blocks.len(),
                });
            }
            None => {}
        }
        let discarded_blocks = self.provisional.len();
        let replay_context = std::mem::take(&mut self.replay_context);
        self.provisional.clear();
        Ok(RollbackOutcome {
            restored_height: anchor.boundary_height,
            restored_root: anchor.boundary_root,
            replay_transactions,
            replay_context,
            discarded_blocks,
        })
    }
}

fn validate_replay_context(
    anchor: &RollbackAnchor,
    provisional: &[FinalizedBlock],
    replay_context: &ReplayContext,
) -> ControllerResult<()> {
    if replay_context.blocks.len() != provisional.len() {
        return Err(ControllerError::ReplayContextIncomplete {
            expected: provisional.len(),
            actual: replay_context.blocks.len(),
        });
    }
    for (block, context) in provisional.iter().zip(replay_context.blocks()) {
        let expected = block.block.header.height;
        if context.height != expected {
            return Err(ControllerError::ReplayContextHeightMismatch {
                expected,
                actual: context.height,
            });
        }
        if context.height <= anchor.boundary_height {
            return Err(ControllerError::ReplayContextHeightMismatch {
                expected: Height::new(anchor.boundary_height.get().saturating_add(1)),
                actual: context.height,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_core::{
        Block, BlockHeader, ChainId, ConfigId, EngineGeneration, EngineId, EngineKind, Epoch,
        FinalityTier,
    };
    use sage_manifest::{
        CertificateKind, CertificatePayload, SignatureEnvelope, SignatureScheme,
        SimulatedSignatureScheme,
    };
    use std::collections::BTreeSet;

    fn certificate(height: Height) -> Certificate {
        let payload = CertificatePayload {
            chain_id: ChainId::new("sage-test"),
            epoch: Epoch::new(1),
            config_id: ConfigId::new(1),
            kind: CertificateKind::Finality,
            height,
            root: Hash32::new([1; 32]),
            block_hash: Some(Hash32::new([2; 32])),
            engine_id: EngineId::new(EngineKind::Poa, EngineGeneration::new(1)),
        };
        let hash = sage_core::crypto::hash_canonical(HashDomain::CertificateV1, &payload);
        Certificate {
            payload,
            signers: BTreeSet::from([sage_core::ValidatorId::new(0)]),
            signature: SimulatedSignatureScheme
                .sign(sage_core::ValidatorId::new(0), hash)
                .unwrap_or(SignatureEnvelope::Simulated { payload_hash: hash }),
        }
    }

    fn anchor(replay_context_root: Option<Hash32>) -> RollbackAnchor {
        RollbackAnchor {
            boundary_height: Height::new(20),
            boundary_root: Hash32::new([3; 32]),
            boundary_hash: Hash32::new([4; 32]),
            legacy_certificate: certificate(Height::new(20)),
            replay_context_root,
        }
    }

    fn replay_block(height: u64, timestamp_micros: u64) -> ReplayBlockContext {
        ReplayBlockContext {
            chain_id: ChainId::new("sage-test"),
            height: Height::new(height),
            timestamp_micros,
            beneficiary: Hash32::new([5; 32]),
            base_fee: 7,
            randomness: Hash32::new([8; 32]),
            oracle_snapshot_root: Some(Hash32::new([9; 32])),
        }
    }

    fn finalized_block(height: u64) -> FinalizedBlock {
        FinalizedBlock {
            block: Block {
                header: BlockHeader {
                    chain_id: ChainId::new("sage-test"),
                    epoch: Epoch::new(1),
                    config_id: ConfigId::new(1),
                    height: Height::new(height),
                    parent_hash: Hash32::new([10; 32]),
                    state_root: Hash32::new([height as u8; 32]),
                    engine_id: EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1)),
                    finality_tier: FinalityTier::Provisional,
                    manifest_hash: None,
                },
                txs: vec![Transaction {
                    from: 1,
                    to: 2,
                    amount: 3,
                    nonce: height,
                }],
            },
            certificate_hash: Hash32::new([11; 32]),
        }
    }

    #[test]
    fn refuses_rollback_at_deadline() {
        let mut manager = RollbackManager::default();
        let schedule = Schedule {
            h_d: Height::new(10),
            h_c: Height::new(20),
            h_r: Height::new(30),
            kappa: 8,
            tau_blocks: 4,
        };
        assert!(matches!(
            manager.rollback(AbortTrigger::ProgressTimeout, Height::new(30), &schedule),
            Err(ControllerError::RollbackDeadlinePassed { .. })
        ));
    }

    #[test]
    fn replay_context_root_commits_to_metadata() {
        let a = ReplayContext::new(vec![replay_block(21, 1_000)]);
        let b = ReplayContext::new(vec![replay_block(21, 2_000)]);
        assert_ne!(a.root(), b.root());

        let mut other_chain = replay_block(21, 1_000);
        other_chain.chain_id = ChainId::new("sage-other");
        assert_ne!(a.root(), ReplayContext::new(vec![other_chain]).root());
    }

    #[test]
    fn replay_context_size_is_per_block_and_bounded() {
        let with_oracle = replay_block(21, 1_000);
        let mut without_oracle = with_oracle.clone();
        without_oracle.oracle_snapshot_root = None;

        // Encoding includes length prefixes for chain_id and 32-byte fields.
        assert_eq!(without_oracle.encoded_len_for_root(), 122);
        assert_eq!(with_oracle.encoded_len_for_root(), 162);
        assert_eq!(
            with_oracle.encoded_len_for_root() - without_oracle.encoded_len_for_root(),
            40
        );
    }

    #[test]
    fn rollback_carries_manifest_bound_replay_context() {
        let schedule = rollback_schedule();
        let context = replay_block(21, 1_000);
        let expected_root = ReplayContext::new(vec![context.clone()]).root();
        let mut manager = RollbackManager::default();
        manager.install_anchor(anchor(Some(expected_root)));
        manager.record_replay_context(context.clone());
        manager.record_provisional(finalized_block(21));

        let outcome = manager
            .rollback(AbortTrigger::ProgressTimeout, Height::new(25), &schedule)
            .unwrap();

        assert_eq!(outcome.restored_height, Height::new(20));
        assert_eq!(outcome.replay_context.blocks(), &[context]);
        assert_eq!(outcome.replay_context.root(), expected_root);
        assert_eq!(outcome.replay_transactions.len(), 1);
    }

    #[test]
    fn rollback_rejects_missing_replay_context() {
        let mut manager = RollbackManager::default();
        manager.install_anchor(anchor(None));
        manager.record_provisional(finalized_block(21));

        assert!(matches!(
            manager.rollback(
                AbortTrigger::ProgressTimeout,
                Height::new(25),
                &rollback_schedule()
            ),
            Err(ControllerError::ReplayContextIncomplete {
                expected: 1,
                actual: 0
            })
        ));
    }

    #[test]
    fn rollback_rejects_replay_context_for_wrong_height() {
        let mut manager = RollbackManager::default();
        manager.install_anchor(anchor(None));
        manager.record_replay_context(replay_block(22, 1_000));
        manager.record_provisional(finalized_block(21));

        assert!(matches!(
            manager.rollback(AbortTrigger::ProgressTimeout, Height::new(25), &rollback_schedule()),
            Err(ControllerError::ReplayContextHeightMismatch {
                expected,
                actual
            }) if expected == Height::new(21) && actual == Height::new(22)
        ));
    }

    #[test]
    fn rejected_rollback_preserves_evidence_for_retry() {
        let mut manager = RollbackManager::default();
        manager.install_anchor(anchor(None));
        manager.record_replay_context(replay_block(22, 1_000));
        manager.record_provisional(finalized_block(21));

        for _ in 0..2 {
            assert!(matches!(
                manager.rollback(
                    AbortTrigger::ProgressTimeout,
                    Height::new(25),
                    &rollback_schedule()
                ),
                Err(ControllerError::ReplayContextHeightMismatch {
                    expected,
                    actual
                }) if expected == Height::new(21) && actual == Height::new(22)
            ));
        }

        assert_eq!(manager.replay_context.blocks().len(), 1);
        assert_eq!(manager.provisional.len(), 1);
    }

    #[test]
    fn rollback_rejects_unbound_replay_context_metadata() {
        let mut manager = RollbackManager::default();
        manager.install_anchor(anchor(None));
        manager.record_replay_context(replay_block(21, 1_000));
        manager.record_provisional(finalized_block(21));

        assert!(matches!(
            manager.rollback(
                AbortTrigger::ProgressTimeout,
                Height::new(25),
                &rollback_schedule()
            ),
            Err(ControllerError::MissingReplayContextRoot { blocks: 1 })
        ));
    }

    fn rollback_schedule() -> Schedule {
        Schedule {
            h_d: Height::new(10),
            h_c: Height::new(20),
            h_r: Height::new(30),
            kappa: 8,
            tau_blocks: 4,
        }
    }
}
