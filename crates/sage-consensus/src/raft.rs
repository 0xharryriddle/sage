//! Crash-tolerant Raft-style legacy engine (log replication under a stable
//! leader) conforming to the `ConsensusEngine` trait.
//!
//! This is the second legacy-engine instantiation, added to make the paper's
//! "engine-agnostic" claim (Section on Generality) a demonstrated artifact
//! rather than prose: SAGE's boundary-crossing construction and proofs quantify
//! over any engine exposing a single-finalizer rule, a deterministic state
//! root, a shadow-usable validation routine, and a root-seeded init. A
//! Raft-to-HotStuff migration is therefore supported by the SAME controller and
//! the SAME proofs, the only change being the legacy quorum family.
//!
//! Faithful-but-minimal Raft modeling (deliberately distinct from `poa.rs`):
//! - A STABLE leader per term (`leader = term % n`) proposes at every height,
//!   rather than PoA's per-height proposer rotation. This mirrors Raft's
//!   single elected leader replicating the log across many entries.
//! - Replication is `AppendEntries` (leader -> followers) answered by
//!   `AppendEntriesAck`; an entry commits once a MAJORITY of the cluster has
//!   acked it (Raft's commit rule), which is the same crash-tolerant
//!   honest-majority fault model as PoA (`QuorumPolicy::Majority`).
//! - The term is carried on the wire so a stale-term message is rejected,
//!   modeling Raft's term-monotonicity guard.
//!
//! What is intentionally NOT modeled (out of scope, same as the rest of the
//! deterministic core): leader election via `RequestVote`, log-conflict
//! back-tracking, and snapshotting. The migration construction depends only on
//! the finalized-prefix single-finalizer property, which this engine provides.

use crate::engine::{BootstrapAnchor, ConsensusEngine, ProposeContext, ShadowVerdict};
use crate::{
    ConsensusError, ConsensusMessage, ConsensusResult, MessageEnvelope, QuorumPolicy, ValidatorSet,
};
use sage_core::crypto::{hash_domain, HashDomain};
use sage_core::{
    Block, BlockHash, BlockHeader, EngineId, FinalityTier, FinalizedBlock, Hash32, StateRoot,
    ValidatorId,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Raft wire messages carried inside `ConsensusMessage::Raft`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaftMessage {
    /// Leader replicates a proposed block (one log entry) to followers.
    AppendEntries { term: u64, block: Block },
    /// Follower acknowledges having appended the entry to its log.
    AppendEntriesAck { term: u64, block_hash: BlockHash },
}

#[derive(Debug, Clone)]
pub struct RaftEngine {
    local_id: ValidatorId,
    engine_id: EngineId,
    validators: ValidatorSet,
    /// Current Raft term; the stable leader is `term % n`.
    current_term: u64,
    last_finalized_hash: BlockHash,
    /// Acks accumulated per replicated entry, keyed by block hash.
    pending_acks: BTreeMap<BlockHash, (Block, BTreeSet<ValidatorId>)>,
    finalized: Vec<FinalizedBlock>,
}

impl RaftEngine {
    pub fn new(
        local_id: ValidatorId,
        engine_id: EngineId,
        validators: ValidatorSet,
        genesis_hash: BlockHash,
    ) -> Self {
        Self {
            local_id,
            engine_id,
            validators,
            current_term: 0,
            last_finalized_hash: genesis_hash,
            pending_acks: BTreeMap::new(),
            finalized: Vec::new(),
        }
    }

    /// Construct at a specific term, so tests can pin which validator leads.
    pub fn with_term(mut self, term: u64) -> Self {
        self.current_term = term;
        self
    }

    pub fn current_term(&self) -> u64 {
        self.current_term
    }

    /// Raft's stable leader for the current term (contrast PoA's per-height
    /// rotation): the same validator replicates every entry until a term change.
    fn leader_for_term(&self, term: u64) -> ValidatorId {
        let n = self.validators.len().max(1) as u64;
        ValidatorId::new((term % n) as u32)
    }

    fn certificate_hash(block_hash: BlockHash, signers: &BTreeSet<ValidatorId>) -> Hash32 {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(block_hash.as_bytes());
        for signer in signers {
            bytes.extend_from_slice(&signer.get().to_be_bytes());
        }
        hash_domain(HashDomain::CertificateV1, &bytes)
    }
}

impl ConsensusEngine for RaftEngine {
    fn engine_id(&self) -> EngineId {
        self.engine_id
    }

    fn quorum_policy(&self) -> QuorumPolicy {
        // Raft commits an entry once a majority of the cluster has appended it:
        // the same crash-tolerant honest-majority family as PoA.
        QuorumPolicy::Majority
    }

    fn init(&mut self, anchor: BootstrapAnchor) -> ConsensusResult<()> {
        self.last_finalized_hash = anchor.certificate_hash;
        self.pending_acks.clear();
        self.finalized.clear();
        Ok(())
    }

    fn propose(&mut self, ctx: ProposeContext<'_>) -> ConsensusResult<Option<Block>> {
        // Only the current term's stable leader proposes.
        if self.leader_for_term(self.current_term) != self.local_id {
            return Ok(None);
        }
        let mut next = ctx.state.clone();
        for tx in ctx.txs {
            next.execution
                .apply_tx(tx)
                .map_err(|e| ConsensusError::InvalidProposal {
                    height: ctx.height,
                    reason: e.to_string(),
                })?;
        }
        let header = BlockHeader {
            chain_id: ctx.state.platform.chain_id.clone(),
            epoch: ctx.state.platform.epoch,
            config_id: ctx.state.platform.config_id,
            height: ctx.height,
            parent_hash: ctx.parent_hash,
            state_root: next.root(),
            engine_id: self.engine_id,
            finality_tier: FinalityTier::Absolute,
            manifest_hash: None,
        };
        Ok(Some(Block {
            header,
            txs: ctx.txs.to_vec(),
        }))
    }

    fn handle_message(&mut self, msg: MessageEnvelope) -> ConsensusResult<Vec<MessageEnvelope>> {
        match msg.message {
            ConsensusMessage::Raft(RaftMessage::AppendEntries { term, block }) => {
                // Reject a stale-term leader (Raft term-monotonicity guard).
                if term < self.current_term {
                    return Err(ConsensusError::InvalidProposal {
                        height: block.header.height,
                        reason: format!("stale Raft term {term} < {}", self.current_term),
                    });
                }
                // Adopt a newer term (a new leader has taken over).
                if term > self.current_term {
                    self.current_term = term;
                }
                // The entry must come from the term's leader.
                if self.leader_for_term(term) != msg.from {
                    return Err(ConsensusError::InvalidProposal {
                        height: block.header.height,
                        reason: "AppendEntries from non-leader".to_string(),
                    });
                }
                let ack = MessageEnvelope {
                    from: self.local_id,
                    to: None,
                    chain_id: msg.chain_id,
                    epoch: msg.epoch,
                    config_id: msg.config_id,
                    message: ConsensusMessage::Raft(RaftMessage::AppendEntriesAck {
                        term,
                        block_hash: block.hash(),
                    }),
                };
                // Buffer the leader's own entry so its self-ack can commit it.
                self.pending_acks
                    .entry(block.hash())
                    .or_insert_with(|| (block, BTreeSet::new()));
                Ok(vec![ack])
            }
            ConsensusMessage::Raft(RaftMessage::AppendEntriesAck { term, block_hash }) => {
                if term < self.current_term {
                    return Ok(Vec::new());
                }
                if let Some((_, signers)) = self.pending_acks.get_mut(&block_hash) {
                    signers.insert(msg.from);
                }
                Ok(Vec::new())
            }
            _ => Ok(Vec::new()),
        }
    }

    fn try_finalize(&mut self) -> ConsensusResult<Vec<FinalizedBlock>> {
        let threshold = self
            .quorum_policy()
            .threshold(&self.validators)?
            .required_power;
        let mut newly_finalized = Vec::new();
        let ready: Vec<_> = self
            .pending_acks
            .iter()
            .filter_map(|(hash, (block, signers))| {
                let power = self.validators.power_of_signers(signers).ok()?;
                (power >= threshold).then_some((*hash, block.clone(), signers.clone()))
            })
            .collect();
        for (hash, block, signers) in ready {
            self.pending_acks.remove(&hash);
            self.last_finalized_hash = hash;
            let finalized = FinalizedBlock {
                block,
                certificate_hash: Self::certificate_hash(hash, &signers),
            };
            self.finalized.push(finalized.clone());
            newly_finalized.push(finalized);
        }
        Ok(newly_finalized)
    }

    fn shadow_validate(&self, block: &Block, state: &sage_core::ChainState) -> ShadowVerdict {
        let mut next = state.clone();
        for tx in &block.txs {
            if let Err(err) = next.execution.apply_tx(tx) {
                return ShadowVerdict::invalid(err.to_string());
            }
        }
        let root: StateRoot = next.root();
        if root == block.header.state_root {
            ShadowVerdict::valid(root)
        } else {
            ShadowVerdict::invalid("Raft shadow root mismatch")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotstuff::HotStuffEngine;
    use sage_core::{
        ChainId, ConfigId, ConsensusStateEnvelope, EngineGeneration, EngineKind, Epoch,
        ExecutionState, Height, PlatformState, Transaction,
    };

    fn engine_id() -> EngineId {
        EngineId::new(EngineKind::Raft, EngineGeneration::new(1))
    }

    fn state(eid: EngineId) -> sage_core::ChainState {
        sage_core::ChainState {
            execution: ExecutionState::new_with_accounts(4, 100),
            platform: PlatformState {
                chain_id: ChainId::new("sage"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                migration: None,
            },
            consensus: ConsensusStateEnvelope {
                engine: eid,
                opaque: vec![],
            },
        }
    }

    fn envelope(from: ValidatorId, message: ConsensusMessage) -> MessageEnvelope {
        MessageEnvelope {
            from,
            to: None,
            chain_id: ChainId::new("sage"),
            epoch: Epoch::new(1),
            config_id: ConfigId::new(1),
            message,
        }
    }

    fn txs() -> Vec<Transaction> {
        vec![Transaction {
            from: 0,
            to: 1,
            amount: 5,
            nonce: 0,
        }]
    }

    #[test]
    fn stable_term_leader_proposes_not_per_height_rotation() {
        let validators = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 4);
        // term 0 => leader is validator 0.
        let mut leader = RaftEngine::new(
            ValidatorId::new(0),
            engine_id(),
            validators.clone(),
            Hash32::ZERO,
        );
        // Leader proposes at height 1 AND height 2 (stable across heights).
        for h in [1u64, 2] {
            let block = leader
                .propose(ProposeContext {
                    state: &state(engine_id()),
                    parent_hash: Hash32::ZERO,
                    height: Height::new(h),
                    txs: &txs(),
                })
                .unwrap();
            assert!(block.is_some(), "term leader must propose at every height");
        }
        // A non-leader (validator 1) never proposes under term 0.
        let mut follower =
            RaftEngine::new(ValidatorId::new(1), engine_id(), validators, Hash32::ZERO);
        let none = follower
            .propose(ProposeContext {
                state: &state(engine_id()),
                parent_hash: Hash32::ZERO,
                height: Height::new(1),
                txs: &txs(),
            })
            .unwrap();
        assert!(none.is_none(), "non-leader must not propose");
    }

    #[test]
    fn majority_acks_finalize_entry() {
        let validators = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 4);
        let mut leader =
            RaftEngine::new(ValidatorId::new(0), engine_id(), validators, Hash32::ZERO);
        let block = leader
            .propose(ProposeContext {
                state: &state(engine_id()),
                parent_hash: Hash32::ZERO,
                height: Height::new(1),
                txs: &txs(),
            })
            .unwrap()
            .unwrap();
        let bh = block.hash();

        // Leader replicates its own entry (self-append) then collects acks.
        leader
            .handle_message(envelope(
                ValidatorId::new(0),
                ConsensusMessage::Raft(RaftMessage::AppendEntries {
                    term: 0,
                    block: block.clone(),
                }),
            ))
            .unwrap();
        // Majority of 4 = 3 acks (validators 0,1,2).
        for v in [0u32, 1, 2] {
            leader
                .handle_message(envelope(
                    ValidatorId::new(v),
                    ConsensusMessage::Raft(RaftMessage::AppendEntriesAck {
                        term: 0,
                        block_hash: bh,
                    }),
                ))
                .unwrap();
        }
        let finalized = leader.try_finalize().unwrap();
        assert_eq!(finalized.len(), 1, "majority acks must commit the entry");
        assert_eq!(finalized[0].block.hash(), bh);
    }

    #[test]
    fn minority_acks_do_not_finalize() {
        let validators = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 4);
        let mut leader =
            RaftEngine::new(ValidatorId::new(0), engine_id(), validators, Hash32::ZERO);
        let block = leader
            .propose(ProposeContext {
                state: &state(engine_id()),
                parent_hash: Hash32::ZERO,
                height: Height::new(1),
                txs: &txs(),
            })
            .unwrap()
            .unwrap();
        let bh = block.hash();
        leader
            .handle_message(envelope(
                ValidatorId::new(0),
                ConsensusMessage::Raft(RaftMessage::AppendEntries { term: 0, block }),
            ))
            .unwrap();
        // Only 2 of 4 ack: below majority (3).
        for v in [0u32, 1] {
            leader
                .handle_message(envelope(
                    ValidatorId::new(v),
                    ConsensusMessage::Raft(RaftMessage::AppendEntriesAck {
                        term: 0,
                        block_hash: bh,
                    }),
                ))
                .unwrap();
        }
        assert!(
            leader.try_finalize().unwrap().is_empty(),
            "a minority of acks must not commit"
        );
    }

    #[test]
    fn stale_term_append_entries_rejected() {
        let validators = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 4);
        // Local engine already at term 2.
        let mut node = RaftEngine::new(ValidatorId::new(1), engine_id(), validators, Hash32::ZERO)
            .with_term(2);
        let block = Block {
            header: BlockHeader {
                chain_id: ChainId::new("sage"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                height: Height::new(1),
                parent_hash: Hash32::ZERO,
                state_root: Hash32::ZERO,
                engine_id: engine_id(),
                finality_tier: FinalityTier::Absolute,
                manifest_hash: None,
            },
            txs: vec![],
        };
        // A leader from stale term 0 tries to append: must be rejected.
        let err = node
            .handle_message(envelope(
                ValidatorId::new(0),
                ConsensusMessage::Raft(RaftMessage::AppendEntries { term: 0, block }),
            ))
            .unwrap_err();
        assert!(matches!(err, ConsensusError::InvalidProposal { .. }));
    }

    #[test]
    fn non_leader_append_entries_rejected() {
        let validators = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 4);
        let mut node = RaftEngine::new(ValidatorId::new(2), engine_id(), validators, Hash32::ZERO);
        let block = Block {
            header: BlockHeader {
                chain_id: ChainId::new("sage"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                height: Height::new(1),
                parent_hash: Hash32::ZERO,
                state_root: Hash32::ZERO,
                engine_id: engine_id(),
                finality_tier: FinalityTier::Absolute,
                manifest_hash: None,
            },
            txs: vec![],
        };
        // term 0 leader is validator 0; validator 3 is NOT the leader.
        let err = node
            .handle_message(envelope(
                ValidatorId::new(3),
                ConsensusMessage::Raft(RaftMessage::AppendEntries { term: 0, block }),
            ))
            .unwrap_err();
        assert!(matches!(err, ConsensusError::InvalidProposal { .. }));
    }

    /// The engine-agnostic migration claim at the trait boundary: a HotStuff
    /// TARGET engine bootstraps (`init`) from a Raft-finalized boundary anchor
    /// exactly as it does from a PoA one — the target only consumes a root +
    /// certificate hash, never anything Raft-specific. This is the interface
    /// property the Generality section relies on for Raft->HotStuff.
    #[test]
    fn hotstuff_target_bootstraps_from_raft_finalized_anchor() {
        let validators = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 4);
        // Drive Raft to finalize one entry, capturing its boundary root+cert.
        let mut leader = RaftEngine::new(
            ValidatorId::new(0),
            engine_id(),
            validators.clone(),
            Hash32::ZERO,
        );
        let block = leader
            .propose(ProposeContext {
                state: &state(engine_id()),
                parent_hash: Hash32::ZERO,
                height: Height::new(1),
                txs: &txs(),
            })
            .unwrap()
            .unwrap();
        let bh = block.hash();
        let boundary_root = block.header.state_root;
        leader
            .handle_message(envelope(
                ValidatorId::new(0),
                ConsensusMessage::Raft(RaftMessage::AppendEntries {
                    term: 0,
                    block: block.clone(),
                }),
            ))
            .unwrap();
        for v in [0u32, 1, 2] {
            leader
                .handle_message(envelope(
                    ValidatorId::new(v),
                    ConsensusMessage::Raft(RaftMessage::AppendEntriesAck {
                        term: 0,
                        block_hash: bh,
                    }),
                ))
                .unwrap();
        }
        let finalized = leader.try_finalize().unwrap();
        assert_eq!(finalized.len(), 1);
        let anchor = BootstrapAnchor {
            height: Height::new(1),
            root: boundary_root,
            certificate_hash: finalized[0].certificate_hash,
        };

        // The HotStuff target consumes the Raft-produced anchor with no change.
        let target_id = EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1));
        let mut target = HotStuffEngine::new(ValidatorId::new(0), target_id, validators, 1);
        assert!(
            target.init(anchor).is_ok(),
            "HotStuff must bootstrap from a Raft-finalized boundary anchor unchanged"
        );
    }
}
