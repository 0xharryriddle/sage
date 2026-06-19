//! Minimal HotStuff BFT engine with timeout-certificate pacemaker.
//!
//! Supported message types:
//! - `Proposal` (leader → all): propose a block at current view
//! - `Vote` (validator → all): vote for a proposal
//! - `Timeout` (validator → all): signal timeout at current view
//! - `NewView` (validator → all): announce view advance with optional timeout certificate
//!
//! Pacemaker rules:
//! - A validator times out once per view (idempotent via `timed_out` flag).
//! - Collecting `2f+1` timeout messages for view `v` forms a `TimeoutCertificate`.
//! - Forming or receiving a TC advances the view to `v+1` and triggers a NewView broadcast.
//! - On receiving a *higher* TC, a validator jumps directly to that view.

use crate::engine::{BootstrapAnchor, ConsensusEngine, ProposeContext, ShadowVerdict};
use crate::{
    ConsensusError, ConsensusMessage, ConsensusResult, MessageEnvelope, QuorumPolicy, ValidatorSet,
};
use sage_core::crypto::{hash_domain, HashDomain};
use sage_core::{
    Block, BlockHash, BlockHeader, EngineId, FinalityTier, FinalizedBlock, Hash32, ValidatorId,
    View,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumCertificate {
    pub view: View,
    pub block_hash: BlockHash,
    pub signers: BTreeSet<ValidatorId>,
    pub certificate_hash: Hash32,
}

impl QuorumCertificate {
    fn qc_hash(view: View, block_hash: BlockHash, signers: &BTreeSet<ValidatorId>) -> Hash32 {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&view.get().to_be_bytes());
        bytes.extend_from_slice(block_hash.as_bytes());
        for signer in signers {
            bytes.extend_from_slice(&signer.get().to_be_bytes());
        }
        hash_domain(HashDomain::CertificateV1, &bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeoutCertificate {
    pub view: View,
    pub signers: BTreeSet<ValidatorId>,
    pub highest_qc: Option<QuorumCertificate>,
    pub certificate_hash: Hash32,
}

impl TimeoutCertificate {
    fn hash(view: View, signers: &BTreeSet<ValidatorId>) -> Hash32 {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&view.get().to_be_bytes());
        for signer in signers {
            bytes.extend_from_slice(&signer.get().to_be_bytes());
        }
        hash_domain(HashDomain::TimeoutCertificateV1, &bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotStuffMessage {
    NewView {
        view: View,
        highest_qc: Option<QuorumCertificate>,
        tc: Option<TimeoutCertificate>,
    },
    Proposal {
        view: View,
        block: Block,
        justify: Option<QuorumCertificate>,
    },
    Vote {
        view: View,
        block: Block,
        block_hash: BlockHash,
    },
    Timeout {
        view: View,
        highest_qc: Option<QuorumCertificate>,
    },
}

#[derive(Debug, Clone)]
pub struct HotStuffEngine {
    local_id: ValidatorId,
    engine_id: EngineId,
    validators: ValidatorSet,
    f: u64,
    view: View,
    highest_qc: Option<QuorumCertificate>,
    last_voted_view: Option<View>,
    pending_votes: BTreeMap<BlockHash, (View, Block, BTreeSet<ValidatorId>)>,
    finalized: Vec<FinalizedBlock>,
    view_bump_count: u64,
    // Pacemaker state
    pending_timeouts: BTreeMap<View, (BTreeSet<ValidatorId>, Option<QuorumCertificate>)>,
    timed_out: bool,
}

impl HotStuffEngine {
    pub fn new(
        local_id: ValidatorId,
        engine_id: EngineId,
        validators: ValidatorSet,
        f: u64,
    ) -> Self {
        Self {
            local_id,
            engine_id,
            validators,
            f,
            view: View::new(0),
            highest_qc: None,
            last_voted_view: None,
            pending_votes: BTreeMap::new(),
            finalized: Vec::new(),
            view_bump_count: 0,
            pending_timeouts: BTreeMap::new(),
            timed_out: false,
        }
    }

    fn leader_for(&self, view: View) -> ValidatorId {
        let n = self.validators.len().max(1) as u64;
        ValidatorId::new((view.get() % n) as u32)
    }

    pub fn bump_view(&mut self) {
        if self.view_bump_count >= 3 {
            return;
        }
        if let Some(next) = self.view.get().checked_add(1) {
            self.view = View::new(next);
            self.pending_votes.clear();
            self.last_voted_view = None;
            self.view_bump_count += 1;
        }
    }

    pub fn bumps_exhausted(&self) -> bool {
        self.view_bump_count >= 3
    }

    pub fn set_view(&mut self, view: View) {
        self.view = view;
        self.pending_votes.clear();
        self.last_voted_view = None;
        self.view_bump_count = 0;
        // Reset timeout state when view changes
        self.timed_out = false;
    }

    pub fn current_view(&self) -> View {
        self.view
    }

    /// Return whether this engine has already emitted a timeout for the
    /// current view (pacemaker idempotency guard).
    pub fn has_timed_out(&self) -> bool {
        self.timed_out
    }

    // ── Pacemaker ──────────────────────────────────────────────────────

    /// Produce a `Timeout` message for the current view (idempotent).
    /// The simulator calls this when a view has been active without finalization.
    pub fn emit_timeout(&mut self) -> Option<ConsensusMessage> {
        if self.timed_out {
            return None;
        }
        self.timed_out = true;
        Some(ConsensusMessage::HotStuff(HotStuffMessage::Timeout {
            view: self.view,
            highest_qc: self.highest_qc.clone(),
        }))
    }

    /// Check whether enough timeouts have been collected to form a TC.
    /// If so, advance the view and return a `NewView` message with the TC.
    /// Returns `None` if no advancement is possible yet.
    pub fn try_advance_view(&mut self) -> ConsensusResult<Option<(View, ConsensusMessage)>> {
        let threshold = self
            .quorum_policy()
            .threshold(&self.validators)?
            .required_power;

        // Collect all views in pending_timeouts where we might advance
        let to_advance: Vec<(View, BTreeSet<ValidatorId>, Option<QuorumCertificate>)> = self
            .pending_timeouts
            .iter()
            .filter_map(|(v, (signers, hqc))| {
                if *v < self.view {
                    return None;
                }
                let power = self.validators.power_of_signers(signers).ok();
                power
                    .filter(|p| *p >= threshold)
                    .map(|_| (*v, signers.clone(), hqc.clone()))
            })
            .collect();

        for (v, signers, highest_qc) in to_advance {
            if v < self.view {
                continue;
            }
            self.pending_timeouts.remove(&v);
            let certificate_hash = TimeoutCertificate::hash(v, &signers);
            let tc = TimeoutCertificate {
                view: v,
                signers,
                highest_qc: highest_qc.clone(),
                certificate_hash,
            };
            // Advance to view v+1
            let next_view = View::new(v.get().saturating_add(1));
            self.view = next_view;
            self.last_voted_view = None;
            self.pending_votes.clear();
            self.timed_out = false;
            self.view_bump_count = 0;
            // Update highest_qc if the TC carried a better one
            if self.highest_qc.is_none() && highest_qc.is_some() {
                self.highest_qc = highest_qc;
            }
            let new_view = ConsensusMessage::HotStuff(HotStuffMessage::NewView {
                view: next_view,
                highest_qc: self.highest_qc.clone(),
                tc: Some(tc),
            });
            return Ok(Some((next_view, new_view)));
        }
        Ok(None)
    }
}

impl ConsensusEngine for HotStuffEngine {
    fn engine_id(&self) -> EngineId {
        self.engine_id
    }
    fn quorum_policy(&self) -> QuorumPolicy {
        QuorumPolicy::Bft { f: self.f }
    }

    fn init(&mut self, anchor: BootstrapAnchor) -> ConsensusResult<()> {
        self.view = View::new(anchor.height.get());
        self.highest_qc = Some(QuorumCertificate {
            view: self.view,
            block_hash: anchor.certificate_hash,
            signers: BTreeSet::new(),
            certificate_hash: anchor.certificate_hash,
        });
        self.last_voted_view = None;
        self.pending_votes.clear();
        self.finalized.clear();
        self.view_bump_count = 0;
        self.pending_timeouts.clear();
        self.timed_out = false;
        Ok(())
    }

    fn propose(&mut self, ctx: ProposeContext<'_>) -> ConsensusResult<Option<Block>> {
        if self.leader_for(self.view) != self.local_id {
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
            finality_tier: FinalityTier::Provisional,
            manifest_hash: None,
        };
        Ok(Some(Block {
            header,
            txs: ctx.txs.to_vec(),
        }))
    }

    fn handle_message(&mut self, msg: MessageEnvelope) -> ConsensusResult<Vec<MessageEnvelope>> {
        match msg.message {
            ConsensusMessage::HotStuff(HotStuffMessage::Proposal {
                view,
                block,
                justify,
            }) => {
                if view > self.view {
                    self.view = view;
                    self.last_voted_view = None;
                    self.timed_out = false;
                }
                if view < self.view {
                    return Ok(Vec::new());
                }
                if self.leader_for(view) != msg.from {
                    return Err(ConsensusError::InvalidProposal {
                        height: block.header.height,
                        reason: "wrong HotStuff leader".to_string(),
                    });
                }
                if self.last_voted_view == Some(view) {
                    return Err(ConsensusError::DoubleVoteAttempt {
                        validator: self.local_id,
                        view,
                    });
                }
                if let Some(qc) = justify {
                    if self.highest_qc.is_none() || qc.view > self.highest_qc.as_ref().unwrap().view
                    {
                        self.highest_qc = Some(qc);
                    }
                }
                self.last_voted_view = Some(view);
                let vote = MessageEnvelope {
                    from: self.local_id,
                    to: None,
                    chain_id: msg.chain_id,
                    epoch: msg.epoch,
                    config_id: msg.config_id,
                    message: ConsensusMessage::HotStuff(HotStuffMessage::Vote {
                        view,
                        block: block.clone(),
                        block_hash: block.hash(),
                    }),
                };
                Ok(vec![vote])
            }
            ConsensusMessage::HotStuff(HotStuffMessage::Vote {
                view,
                block,
                block_hash,
            }) => {
                if view < self.view {
                    // Stale vote, ignore
                    return Ok(Vec::new());
                }
                let (_, _, signers) = self
                    .pending_votes
                    .entry(block_hash)
                    .or_insert_with(|| (view, block, BTreeSet::new()));
                signers.insert(msg.from);
                Ok(Vec::new())
            }
            ConsensusMessage::HotStuff(HotStuffMessage::NewView {
                view,
                highest_qc,
                tc,
            }) => {
                if view <= self.view {
                    return Ok(Vec::new());
                }
                // Verify TC if present
                if let Some(tc_val) = &tc {
                    if tc_val.view >= self.view {
                        // Validate TC signers reach threshold
                        let threshold = self.quorum_policy().threshold(&self.validators)?;
                        let power = self
                            .validators
                            .power_of_signers(&tc_val.signers)
                            .unwrap_or(0);
                        if power >= threshold.required_power {
                            self.view = view;
                            self.last_voted_view = None;
                            self.timed_out = false;
                            self.view_bump_count = 0;
                            if let Some(qc) = highest_qc {
                                self.highest_qc = Some(qc);
                            } else if let Some(qc) = &tc_val.highest_qc {
                                self.highest_qc = Some(qc.clone());
                            }
                        }
                    }
                } else {
                    // Without TC, trust the sender (legacy/fallback)
                    self.view = view;
                    self.last_voted_view = None;
                    self.timed_out = false;
                }
                Ok(Vec::new())
            }
            ConsensusMessage::HotStuff(HotStuffMessage::Timeout { view, highest_qc }) => {
                if view < self.view {
                    return Ok(Vec::new());
                }
                let (signers, hqc) = self
                    .pending_timeouts
                    .entry(view)
                    .or_insert_with(|| (BTreeSet::new(), None));
                signers.insert(msg.from);
                // Keep the highest QC seen among timeout messages
                if highest_qc.is_some()
                    && (hqc.is_none()
                        || highest_qc.as_ref().unwrap().view > hqc.as_ref().unwrap().view)
                {
                    *hqc = highest_qc;
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
        let ready: Vec<_> = self
            .pending_votes
            .iter()
            .filter_map(|(hash, (view, block, signers))| {
                let power = self.validators.power_of_signers(signers).ok()?;
                (power >= threshold).then_some((*hash, *view, block.clone(), signers.clone()))
            })
            .collect();
        let mut out = Vec::new();
        for (hash, view, block, signers) in ready {
            self.pending_votes.remove(&hash);
            let qc = QuorumCertificate {
                view,
                block_hash: hash,
                certificate_hash: QuorumCertificate::qc_hash(view, hash, &signers),
                signers,
            };
            self.highest_qc = Some(qc.clone());
            let finalized = FinalizedBlock {
                block,
                certificate_hash: qc.certificate_hash,
            };
            self.finalized.push(finalized.clone());
            out.push(finalized);
            self.view = self
                .view
                .get()
                .checked_add(1)
                .map(View::new)
                .unwrap_or(self.view);
            // Reset timed_out on view advance via finalization
            self.timed_out = false;
        }
        Ok(out)
    }

    fn shadow_validate(&self, block: &Block, state: &sage_core::ChainState) -> ShadowVerdict {
        crate::hotstuff_shadow::ShadowValidator::validate_block(block, state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_core::{
        ChainId, ConfigId, ConsensusStateEnvelope, EngineGeneration, EngineKind, Epoch,
        ExecutionState, Height, PlatformState, Transaction,
    };

    #[test]
    fn leader_proposes_provisional_block() {
        let engine_id = EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1));
        let validators = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 4);
        let mut engine = HotStuffEngine::new(ValidatorId::new(0), engine_id, validators, 1);
        let state = sage_core::ChainState {
            execution: ExecutionState::new_with_accounts(2, 10),
            platform: PlatformState {
                chain_id: ChainId::new("sage"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                migration: None,
            },
            consensus: ConsensusStateEnvelope {
                engine: engine_id,
                opaque: vec![],
            },
        };
        let txs = [Transaction {
            from: 0,
            to: 1,
            amount: 1,
            nonce: 0,
        }];
        let block = engine
            .propose(ProposeContext {
                state: &state,
                parent_hash: Hash32::ZERO,
                height: Height::new(1),
                txs: &txs,
            })
            .unwrap()
            .unwrap();
        assert_eq!(block.header.finality_tier, FinalityTier::Provisional);
    }

    #[test]
    fn timeout_idempotent() {
        let validators = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 6);
        let mut engine = HotStuffEngine::new(
            ValidatorId::new(0),
            EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1)),
            validators,
            1,
        );

        // First call returns Timeout
        assert!(engine.emit_timeout().is_some());
        // Second call is idempotent
        assert!(engine.emit_timeout().is_none());
        // After view change, can timeout again
        engine.set_view(View::new(1));
        assert!(engine.emit_timeout().is_some());
    }

    #[test]
    fn timeout_certificate_formation() {
        let validators = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 6);
        let mut engine = HotStuffEngine::new(
            ValidatorId::new(0),
            EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1)),
            validators,
            1, // f=1, threshold = 2*1+1 = 3
        );

        // Simulate receiving 3 Timeout messages for view 0
        // (we need 2f+1 = 3 to form TC)
        for id in 0..3u32 {
            let timeout_msg = MessageEnvelope {
                from: ValidatorId::new(id),
                to: None,
                chain_id: ChainId::new("sage"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                message: ConsensusMessage::HotStuff(HotStuffMessage::Timeout {
                    view: View::new(0),
                    highest_qc: None,
                }),
            };
            engine.handle_message(timeout_msg).unwrap();
        }

        // Now try_advance_view should produce a NewView
        let result = engine.try_advance_view().unwrap();
        assert!(result.is_some(), "3 timeouts should form TC for view 0");
        let (new_view, _msg) = result.unwrap();
        assert_eq!(new_view, View::new(1));
    }

    #[test]
    fn no_timeout_advance_below_threshold() {
        let validators = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 6);
        let mut engine = HotStuffEngine::new(
            ValidatorId::new(0),
            EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1)),
            validators,
            1, // threshold=3
        );

        // Only 2 timeouts (below threshold)
        for id in 0..2u32 {
            let timeout_msg = MessageEnvelope {
                from: ValidatorId::new(id),
                to: None,
                chain_id: ChainId::new("sage"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                message: ConsensusMessage::HotStuff(HotStuffMessage::Timeout {
                    view: View::new(0),
                    highest_qc: None,
                }),
            };
            engine.handle_message(timeout_msg).unwrap();
        }

        let result = engine.try_advance_view().unwrap();
        assert!(result.is_none());
    }
}
