use crate::crypto::{
    encode_bytes, encode_u32, encode_u64, hash_canonical, CanonicalEncode, HashDomain,
};
use crate::{
    BlockHash, ChainId, ConfigId, CoreError, CoreResult, EngineId, Epoch, FinalityTier, Hash32,
    Height, StateRoot,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub from: u64,
    pub to: u64,
    pub amount: u64,
    pub nonce: u64,
}

impl CanonicalEncode for Transaction {
    fn encode_canonical(&self, out: &mut Vec<u8>) {
        encode_u64(out, self.from);
        encode_u64(out, self.to);
        encode_u64(out, self.amount);
        encode_u64(out, self.nonce);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub chain_id: ChainId,
    pub epoch: Epoch,
    pub config_id: ConfigId,
    pub height: Height,
    pub parent_hash: BlockHash,
    pub state_root: StateRoot,
    pub engine_id: EngineId,
    pub finality_tier: FinalityTier,
    pub manifest_hash: Option<Hash32>,
}

impl BlockHeader {
    pub fn hash(&self) -> BlockHash {
        hash_canonical(HashDomain::BlockHeaderV1, self)
    }

    pub fn validate_child_of(&self, parent: &BlockHeader) -> CoreResult<()> {
        let expected_height = parent
            .height
            .checked_next()
            .ok_or(CoreError::ArithmeticOverflow("child height"))?;
        if self.height != expected_height {
            return Err(CoreError::InvalidHeight {
                expected: expected_height,
                actual: self.height,
            });
        }

        let expected_parent = parent.hash();
        if self.parent_hash != expected_parent {
            return Err(CoreError::InvalidParent {
                expected: expected_parent,
                actual: self.parent_hash,
            });
        }

        if self.epoch < parent.epoch {
            return Err(CoreError::InvalidEpoch {
                expected: parent.epoch,
                actual: self.epoch,
            });
        }

        Ok(())
    }
}

impl CanonicalEncode for BlockHeader {
    fn encode_canonical(&self, out: &mut Vec<u8>) {
        encode_bytes(out, self.chain_id.as_str().as_bytes());
        encode_u64(out, self.epoch.get());
        encode_u64(out, self.config_id.get());
        encode_u64(out, self.height.get());
        encode_bytes(out, self.parent_hash.as_bytes());
        encode_bytes(out, self.state_root.as_bytes());
        encode_u32(out, self.engine_id.kind as u32);
        encode_u64(out, self.engine_id.generation.get());
        encode_u32(out, self.finality_tier as u32);
        match self.manifest_hash {
            Some(hash) => {
                out.push(1);
                encode_bytes(out, hash.as_bytes());
            }
            None => out.push(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub txs: Vec<Transaction>,
}

impl Block {
    pub fn hash(&self) -> BlockHash {
        hash_canonical(HashDomain::BlockV1, self)
    }
}

impl CanonicalEncode for Block {
    fn encode_canonical(&self, out: &mut Vec<u8>) {
        self.header.encode_canonical(out);
        encode_u64(out, self.txs.len() as u64);
        for tx in &self.txs {
            tx.encode_canonical(out);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalizedBlock {
    pub block: Block,
    pub certificate_hash: Hash32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EngineGeneration, EngineKind};

    fn header(height: u64, parent_hash: BlockHash) -> BlockHeader {
        BlockHeader {
            chain_id: ChainId::new("sage-test"),
            epoch: Epoch::new(1),
            config_id: ConfigId::new(1),
            height: Height::new(height),
            parent_hash,
            state_root: Hash32::new([height as u8; 32]),
            engine_id: EngineId::new(EngineKind::Poa, EngineGeneration::new(1)),
            finality_tier: FinalityTier::Absolute,
            manifest_hash: None,
        }
    }

    #[test]
    fn validates_parent_link() {
        let parent = header(0, Hash32::ZERO);
        let child = header(1, parent.hash());
        assert!(child.validate_child_of(&parent).is_ok());
    }

    #[test]
    fn rejects_wrong_parent_link() {
        let parent = header(0, Hash32::ZERO);
        let child = header(1, Hash32::new([7; 32]));
        assert!(matches!(
            child.validate_child_of(&parent),
            Err(CoreError::InvalidParent { .. })
        ));
    }
}
