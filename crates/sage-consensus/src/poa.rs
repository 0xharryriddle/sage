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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoaMessage {
    Proposal(Block),
    Vote { block: Block, block_hash: BlockHash },
}

#[derive(Debug, Clone)]
pub struct PoaEngine {
    local_id: ValidatorId,
    engine_id: EngineId,
    validators: ValidatorSet,
    last_finalized_hash: BlockHash,
    pending_votes: BTreeMap<BlockHash, (Block, BTreeSet<ValidatorId>)>,
    finalized: Vec<FinalizedBlock>,
}

impl PoaEngine {
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
            last_finalized_hash: genesis_hash,
            pending_votes: BTreeMap::new(),
            finalized: Vec::new(),
        }
    }

    fn proposer_for(&self, height: sage_core::Height) -> ValidatorId {
        let n = self.validators.len().max(1) as u64;
        ValidatorId::new((height.get() % n) as u32)
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

impl ConsensusEngine for PoaEngine {
    fn engine_id(&self) -> EngineId {
        self.engine_id
    }
    fn quorum_policy(&self) -> QuorumPolicy {
        QuorumPolicy::Majority
    }

    fn init(&mut self, anchor: BootstrapAnchor) -> ConsensusResult<()> {
        self.last_finalized_hash = anchor.certificate_hash;
        self.pending_votes.clear();
        self.finalized.clear();
        Ok(())
    }

    fn propose(&mut self, ctx: ProposeContext<'_>) -> ConsensusResult<Option<Block>> {
        if self.proposer_for(ctx.height) != self.local_id {
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
            ConsensusMessage::Poa(PoaMessage::Proposal(block)) => {
                if self.proposer_for(block.header.height) != msg.from {
                    return Err(ConsensusError::InvalidProposal {
                        height: block.header.height,
                        reason: "wrong PoA proposer".to_string(),
                    });
                }
                let vote = MessageEnvelope {
                    from: self.local_id,
                    to: None,
                    chain_id: msg.chain_id,
                    epoch: msg.epoch,
                    config_id: msg.config_id,
                    message: ConsensusMessage::Poa(PoaMessage::Vote {
                        block: block.clone(),
                        block_hash: block.hash(),
                    }),
                };
                Ok(vec![vote])
            }
            ConsensusMessage::Poa(PoaMessage::Vote { block, block_hash }) => {
                let (_, signers) = self
                    .pending_votes
                    .entry(block_hash)
                    .or_insert_with(|| (block, BTreeSet::new()));
                signers.insert(msg.from);
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
            .pending_votes
            .iter()
            .filter_map(|(hash, (block, signers))| {
                let power = self.validators.power_of_signers(signers).ok()?;
                (power >= threshold).then_some((*hash, block.clone(), signers.clone()))
            })
            .collect();
        for (hash, block, signers) in ready {
            self.pending_votes.remove(&hash);
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
            ShadowVerdict::invalid("PoA shadow root mismatch")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_core::{
        ChainId, ConfigId, ConsensusStateEnvelope, EngineGeneration, EngineKind, Epoch,
        ExecutionState, Height, PlatformState, Transaction,
    };

    fn state(engine_id: EngineId) -> sage_core::ChainState {
        sage_core::ChainState {
            execution: ExecutionState::new_with_accounts(4, 100),
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
        }
    }

    #[test]
    fn local_proposer_builds_block_with_expected_root() {
        let engine_id = EngineId::new(EngineKind::Poa, EngineGeneration::new(1));
        let validators = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 4);
        let mut engine = PoaEngine::new(ValidatorId::new(1), engine_id, validators, Hash32::ZERO);
        let txs = [Transaction {
            from: 0,
            to: 1,
            amount: 5,
            nonce: 0,
        }];
        let block = engine
            .propose(ProposeContext {
                state: &state(engine_id),
                parent_hash: Hash32::ZERO,
                height: Height::new(1),
                txs: &txs,
            })
            .unwrap()
            .unwrap();
        assert_eq!(block.header.height, Height::new(1));
        assert!(engine.shadow_validate(&block, &state(engine_id)).valid);
    }
}
