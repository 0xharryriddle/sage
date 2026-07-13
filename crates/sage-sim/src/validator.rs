use crate::SimResult;
use crate::StrategyKind;
use sage_consensus::{
    ConsensusEngine, ConsensusMessage, HotStuffEngine, MessageEnvelope, PoaEngine,
};
use sage_controller::{ReadinessContext, SageController, ShadowRecord};
use sage_core::{Block, ChainState, EngineId, FinalizedBlock, Height, ValidatorId};

#[derive(Debug)]
pub struct SimValidator {
    pub id: ValidatorId,
    pub strategy: StrategyKind,
    pub state: ChainState,
    pub legacy: PoaEngine,
    pub target: HotStuffEngine,
    pub controller: SageController,
    pub committed: Vec<FinalizedBlock>,
    pub(crate) cutover_height: Option<Height>,
}

impl SimValidator {
    pub fn new(
        id: ValidatorId,
        strategy: StrategyKind,
        state: ChainState,
        legacy: PoaEngine,
        target: HotStuffEngine,
        controller: SageController,
    ) -> Self {
        Self {
            id,
            strategy,
            state,
            legacy,
            target,
            controller,
            committed: Vec::new(),
            cutover_height: None,
        }
    }

    pub fn cutover_happened(&self) -> bool {
        self.cutover_height.is_some()
    }

    pub fn snapshot(&self) -> sage_controller::ValidatorSnapshot {
        let next_height = self
            .committed
            .last()
            .map(|b| {
                b.block
                    .header
                    .height
                    .checked_next()
                    .unwrap_or(b.block.header.height)
            })
            .unwrap_or(Height::new(1));
        sage_controller::ValidatorSnapshot {
            validator: self.id,
            correct: true,
            authoritative_engine: self.authoritative_engine(next_height),
            committed: self
                .committed
                .iter()
                .map(|block| sage_controller::CommittedBlockSummary {
                    height: block.block.header.height,
                    block_hash: block.block.hash(),
                    engine: block.block.header.engine_id,
                    finality_tier: block.block.header.finality_tier,
                })
                .collect(),
        }
    }

    fn decide_authority(&mut self, height: Height) -> Option<Height> {
        let schedule = self.controller.schedule;
        match self.strategy {
            StrategyKind::Sage => {
                // cutover was already decided via controller
                None
            }
            StrategyKind::HardFork => {
                if height.get() >= schedule.h_c.get() && self.cutover_height.is_none() {
                    self.cutover_height = Some(schedule.h_c);
                    Some(schedule.h_c)
                } else {
                    None
                }
            }
            StrategyKind::SageBlind => {
                if height.get() >= schedule.h_c.get() && self.cutover_height.is_none() {
                    self.cutover_height = Some(schedule.h_c);
                    Some(schedule.h_c)
                } else {
                    None
                }
            }
            StrategyKind::CoxStyle => {
                // Cox-style live switch at the scheduled cutover height. Like a
                // homogeneous BFT<->BFT switch it forms a boundary checkpoint and
                // switches with no halt; unlike SAGE it does NOT gate on the n-f
                // dual-run matching-root predicate. The cutover height is the
                // same h_c; what differs (and is OBSERVED, not flagged) is the
                // absence of shadow-anchored bootstrap validation across the
                // heterogeneous boundary.
                if height.get() >= schedule.h_c.get() && self.cutover_height.is_none() {
                    self.cutover_height = Some(schedule.h_c);
                    Some(schedule.h_c)
                } else {
                    None
                }
            }
            StrategyKind::StopTheWorld => {
                if height.get() >= schedule.h_r.get() && self.cutover_height.is_none() {
                    self.cutover_height = Some(schedule.h_r);
                    Some(schedule.h_r)
                } else {
                    None
                }
            }
            StrategyKind::ReconfigOnly => None,
        }
    }

    pub(crate) fn authoritative_engine(&self, height: Height) -> EngineId {
        let schedule = self.controller.schedule;
        match self.strategy {
            StrategyKind::Sage => {
                if self.cutover_height.map(|h| height >= h).unwrap_or(false) {
                    self.target.engine_id()
                } else {
                    self.legacy.engine_id()
                }
            }
            StrategyKind::HardFork => {
                if height >= schedule.h_c {
                    self.target.engine_id()
                } else {
                    self.legacy.engine_id()
                }
            }
            StrategyKind::SageBlind => {
                if height >= schedule.h_c {
                    self.target.engine_id()
                } else {
                    self.legacy.engine_id()
                }
            }
            StrategyKind::CoxStyle => {
                // Cox-style live switch: target engine becomes authoritative at
                // h_c (same as the blind/hard cutover), modeling a zero-downtime
                // homogeneous switch applied across the heterogeneous boundary.
                if height >= schedule.h_c {
                    self.target.engine_id()
                } else {
                    self.legacy.engine_id()
                }
            }
            StrategyKind::StopTheWorld => {
                if height >= schedule.h_r {
                    self.target.engine_id()
                } else {
                    self.legacy.engine_id()
                }
            }
            StrategyKind::ReconfigOnly => self.legacy.engine_id(),
        }
    }

    pub fn propose(&mut self, ctx: sage_consensus::ProposeContext<'_>) -> SimResult<Option<Block>> {
        let height = ctx.height;
        self.decide_authority(height);
        let is_legacy = self.authoritative_engine(height).kind == sage_core::EngineKind::Poa;
        if is_legacy {
            Ok(self.legacy.propose(ctx)?)
        } else {
            Ok(self.target.propose(ctx)?)
        }
    }

    pub fn expected_proposer(&self, height: Height, n: u32) -> ValidatorId {
        ValidatorId::new((height.get() % n as u64) as u32)
    }

    pub fn handle_message(&mut self, message: MessageEnvelope) -> SimResult<Vec<MessageEnvelope>> {
        match message.message {
            ConsensusMessage::Poa(_) => Ok(self.legacy.handle_message(message)?),
            ConsensusMessage::HotStuff(_) => {
                // after cutover, also update any state change from legacy engine
                let _ = self.legacy.handle_message(message.clone());
                Ok(self.target.handle_message(message)?)
            }
            // The deterministic simulator drives cutover via the controller
            // directly (one global proposer per height), so it does not exchange
            // SAGE cutover-attestation messages; ignore them here.
            ConsensusMessage::Sage(_) => Ok(Vec::new()),
            // This simulator instantiates PoA as the legacy engine; Raft is a
            // conforming alternative legacy engine covered by its own tests, and
            // does not flow through this PoA->HotStuff simulation path.
            ConsensusMessage::Raft(_) => Ok(Vec::new()),
        }
    }

    pub fn try_finalize(
        &mut self,
        shadow_ctx: Option<ReadinessContext>,
    ) -> SimResult<(Vec<FinalizedBlock>, Vec<Height>, Vec<MessageEnvelope>)> {
        let next_height = self
            .committed
            .last()
            .map(|b| b.block.header.height)
            .unwrap_or(Height::new(0))
            .checked_next()
            .unwrap_or(Height::new(1));
        let authoritative_id = self.authoritative_engine(next_height);

        let legacy_finalized = self.legacy.try_finalize()?;
        let (finalized, using_legacy) = if authoritative_id == self.legacy.engine_id() {
            (legacy_finalized, true)
        } else {
            let target_finalized = self.target.try_finalize()?;
            if target_finalized.is_empty() {
                // View advancement is managed by the simulation's height_attempts/hotstuff_view.
                // The engine-level pacemaker (emit_timeout + try_advance_view) is available but
                // not driven here — it will be used by the real-node (sage-node) runtime where
                // wall-clock timers are available.
                (legacy_finalized, false)
            } else {
                let mut combined = legacy_finalized;
                combined.extend(target_finalized);
                (combined, false)
            }
        };

        // ── Block processing ──
        let mut accepted = Vec::new();
        let mut cutovers = Vec::new();
        for block in finalized {
            let block_height = block.block.header.height;
            // decide authority for non-SAGE strategies
            if let Some(cutover_h) = self.decide_authority(block_height) {
                // bootstrap target engine for non-SAGE cutover
                let _ = self.target.init(sage_consensus::BootstrapAnchor {
                    height: cutover_h,
                    root: block.block.header.state_root,
                    certificate_hash: block.block.hash(),
                });
                // ensure all validators start at same view so partition peers can vote
                self.target.set_view(sage_core::View::new(0));
                cutovers.push(cutover_h);
            }
            if self
                .committed
                .iter()
                .any(|existing| existing.block.header.height == block_height)
            {
                continue;
            }
            // shadow validation for SAGE strategies only during legacy finalization
            if let Some(ctx) = shadow_ctx.clone() {
                if using_legacy && block_height >= self.controller.schedule.h_d {
                    let verdict = self.target.shadow_validate(&block.block, &self.state);
                    let record = ShadowRecord {
                        height: block_height,
                        validator: self.id,
                        verdict_root: verdict.recomputed_root,
                        canonical_root: block.block.header.state_root,
                        valid: verdict.valid,
                    };
                    if let Some(cutover_height) = self.controller.observe_shadow(record, ctx) {
                        self.cutover_height = Some(cutover_height);
                        // bootstrap target engine at cutover
                        let _ = self.target.init(sage_consensus::BootstrapAnchor {
                            height: cutover_height,
                            root: block.block.header.state_root,
                            certificate_hash: block.block.hash(),
                        });
                        self.target.set_view(sage_core::View::new(0));
                        cutovers.push(cutover_height);
                    }
                }
            }
            self.state = self.state.apply_block(&block.block)?;
            self.committed.push(block.clone());
            accepted.push(block);
        }
        Ok((accepted, cutovers, Vec::new()))
    }

    pub fn accepts(&self, message: &MessageEnvelope) -> bool {
        let is_poa = matches!(message.message, ConsensusMessage::Poa(_));
        let is_hotstuff = matches!(message.message, ConsensusMessage::HotStuff(_));
        let addressed_to_me = message.to.map(|to| to == self.id).unwrap_or(true);
        (is_poa || is_hotstuff) && addressed_to_me
    }
}
