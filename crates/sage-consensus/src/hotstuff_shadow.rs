use crate::ShadowVerdict;
use sage_core::{Block, ChainState};

#[derive(Debug, Clone, Copy, Default)]
pub struct ShadowValidator;

impl ShadowValidator {
    pub fn validate_block(block: &Block, state: &ChainState) -> ShadowVerdict {
        let mut next = state.clone();
        for tx in &block.txs {
            if let Err(err) = next.execution.apply_tx(tx) {
                return ShadowVerdict::invalid(err.to_string());
            }
        }
        let root = next.root();
        if root == block.header.state_root {
            ShadowVerdict::valid(root)
        } else {
            ShadowVerdict::invalid("shadow root mismatch")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_core::{
        BlockHeader, ChainId, ConfigId, ConsensusStateEnvelope, EngineGeneration, EngineId,
        EngineKind, Epoch, ExecutionState, FinalityTier, Hash32, Height, PlatformState,
        Transaction,
    };

    #[test]
    fn detects_root_mismatch() {
        let engine = EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1));
        let state = ChainState {
            execution: ExecutionState::new_with_accounts(2, 10),
            platform: PlatformState {
                chain_id: ChainId::new("sage"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                migration: None,
            },
            consensus: ConsensusStateEnvelope {
                engine,
                opaque: vec![],
            },
        };
        let block = Block {
            header: BlockHeader {
                chain_id: ChainId::new("sage"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                height: Height::new(1),
                parent_hash: Hash32::ZERO,
                state_root: Hash32::ZERO,
                engine_id: engine,
                finality_tier: FinalityTier::Provisional,
                manifest_hash: None,
            },
            txs: vec![Transaction {
                from: 0,
                to: 1,
                amount: 1,
                nonce: 0,
            }],
        };
        assert!(!ShadowValidator::validate_block(&block, &state).valid);
    }
}
