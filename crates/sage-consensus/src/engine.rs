use crate::{ConsensusResult, MessageEnvelope, QuorumPolicy};
use sage_core::{
    Block, BlockHash, ChainState, EngineId, FinalizedBlock, Hash32, Height, StateRoot, Transaction,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalizationCertificate {
    pub height: Height,
    pub block_hash: BlockHash,
    pub certificate_hash: Hash32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapAnchor {
    pub height: Height,
    pub root: StateRoot,
    pub certificate_hash: Hash32,
}

#[derive(Debug, Clone)]
pub struct ProposeContext<'a> {
    pub state: &'a ChainState,
    pub parent_hash: BlockHash,
    pub height: Height,
    pub txs: &'a [Transaction],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowVerdict {
    pub valid: bool,
    pub recomputed_root: Option<StateRoot>,
    pub reason: Option<String>,
}

impl ShadowVerdict {
    pub fn valid(root: StateRoot) -> Self {
        Self {
            valid: true,
            recomputed_root: Some(root),
            reason: None,
        }
    }
    pub fn invalid(reason: impl Into<String>) -> Self {
        Self {
            valid: false,
            recomputed_root: None,
            reason: Some(reason.into()),
        }
    }
}

pub trait ConsensusEngine {
    fn engine_id(&self) -> EngineId;
    fn quorum_policy(&self) -> QuorumPolicy;
    fn init(&mut self, anchor: BootstrapAnchor) -> ConsensusResult<()>;
    fn propose(&mut self, ctx: ProposeContext<'_>) -> ConsensusResult<Option<Block>>;
    fn handle_message(&mut self, msg: MessageEnvelope) -> ConsensusResult<Vec<MessageEnvelope>>;
    fn try_finalize(&mut self) -> ConsensusResult<Vec<FinalizedBlock>>;
    fn shadow_validate(&self, block: &Block, state: &ChainState) -> ShadowVerdict;
}
