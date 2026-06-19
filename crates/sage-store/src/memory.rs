//! In-memory storage backend implementing all store traits.
//! Used for tests, simulator integration, and local runtime.
use crate::error::{StoreError, StoreResult};
use crate::traits::{
    BlockStore, CertificateKind, CertificateStore, ManifestStore, SafetyStore, StateStore,
    VoteRecord,
};
use sage_core::{EngineId, Epoch, FinalizedBlock, Height, StateRoot, ValidatorId, View};
use sage_manifest::{Certificate, MigrationManifest};
use std::collections::BTreeMap;

/// In-memory backend implementing all storage traits.
/// Uses BTreeMap for height/epoch-keyed lookups.
#[derive(Debug, Clone, Default)]
pub struct MemoryBackend {
    blocks: BTreeMap<Height, FinalizedBlock>,
    certificates: BTreeMap<CertificateKind, Certificate>,
    manifests: BTreeMap<Epoch, MigrationManifest>,
    states: BTreeMap<Height, StateSnapshot>,
    votes: BTreeMap<(ValidatorId, EngineId, View), VoteRecord>,
}

#[derive(Debug, Clone)]
struct StateSnapshot {
    root: StateRoot,
    data: Vec<u8>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.blocks.clear();
        self.certificates.clear();
        self.manifests.clear();
        self.states.clear();
        self.votes.clear();
    }
}

impl BlockStore for MemoryBackend {
    fn put_block(&mut self, block: FinalizedBlock) -> StoreResult<()> {
        self.blocks.insert(block.block.header.height, block);
        Ok(())
    }

    fn get_block(&self, height: Height) -> StoreResult<&FinalizedBlock> {
        self.blocks
            .get(&height)
            .ok_or(StoreError::BlockNotFound(height))
    }

    fn block_exists(&self, height: Height) -> bool {
        self.blocks.contains_key(&height)
    }

    fn highest_block(&self) -> Option<Height> {
        self.blocks.keys().last().copied()
    }
}

impl CertificateStore for MemoryBackend {
    fn put_certificate(&mut self, kind: CertificateKind, cert: Certificate) -> StoreResult<()> {
        self.certificates.insert(kind, cert);
        Ok(())
    }

    fn get_certificate(&self, kind: &CertificateKind) -> StoreResult<&Certificate> {
        self.certificates
            .get(kind)
            .ok_or(StoreError::CertificateNotFound)
    }
}

impl ManifestStore for MemoryBackend {
    fn put_manifest(&mut self, manifest: MigrationManifest) -> StoreResult<()> {
        self.manifests.insert(manifest.epoch, manifest);
        Ok(())
    }

    fn get_manifest(&self, epoch: Epoch) -> StoreResult<&MigrationManifest> {
        self.manifests
            .get(&epoch)
            .ok_or(StoreError::ManifestNotFound(epoch.get()))
    }
}

impl StateStore for MemoryBackend {
    fn put_state(&mut self, height: Height, root: StateRoot, data: Vec<u8>) -> StoreResult<()> {
        self.states.insert(height, StateSnapshot { root, data });
        Ok(())
    }

    fn get_state(&self, height: Height) -> StoreResult<&[u8]> {
        self.states
            .get(&height)
            .map(|s| s.data.as_slice())
            .ok_or(StoreError::StateNotFound(height))
    }

    fn state_root(&self, height: Height) -> Option<StateRoot> {
        self.states.get(&height).map(|s| s.root)
    }
}

impl SafetyStore for MemoryBackend {
    fn put_vote(&mut self, vote: VoteRecord) -> StoreResult<()> {
        let key = (vote.validator, vote.engine_id, vote.view);
        if let Some(existing) = self.votes.get(&key) {
            if existing.block_hash != vote.block_hash || existing.height != vote.height {
                return Err(StoreError::ConflictingVote);
            }
            return Ok(());
        }
        self.votes.insert(key, vote);
        Ok(())
    }

    fn get_vote(
        &self,
        validator: ValidatorId,
        engine_id: EngineId,
        view: View,
    ) -> Option<&VoteRecord> {
        self.votes.get(&(validator, engine_id, view))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_core::{
        Block, BlockHash, BlockHeader, ChainId, ConfigId, EngineGeneration, EngineId, EngineKind,
        FinalityTier, Hash32,
    };
    use sage_manifest::{Certificate, CertificatePayload, SignatureEnvelope};
    use std::collections::BTreeSet;

    fn dummy_block(height: u64) -> FinalizedBlock {
        let header = BlockHeader {
            chain_id: ChainId::new("test"),
            epoch: Epoch::new(1),
            config_id: ConfigId::new(1),
            height: Height::new(height),
            parent_hash: BlockHash::new([0; 32]),
            state_root: StateRoot::new([height as u8; 32]),
            engine_id: EngineId::new(EngineKind::Poa, EngineGeneration::new(1)),
            finality_tier: FinalityTier::Absolute,
            manifest_hash: None,
        };
        let block = Block {
            header,
            txs: vec![],
        };
        let hash = block.hash();
        FinalizedBlock {
            block,
            certificate_hash: hash,
        }
    }

    fn dummy_cert() -> Certificate {
        Certificate {
            payload: CertificatePayload {
                chain_id: ChainId::new("test"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                kind: sage_manifest::CertificateKind::Finality,
                height: Height::new(1),
                root: StateRoot::new([0; 32]),
                block_hash: None,
                engine_id: EngineId::new(EngineKind::Poa, EngineGeneration::new(1)),
            },
            signers: BTreeSet::new(),
            signature: SignatureEnvelope::Simulated {
                payload_hash: Hash32::new([0; 32]),
            },
        }
    }

    #[test]
    fn put_and_get_block() {
        let mut store = MemoryBackend::new();
        let block = dummy_block(42);
        store.put_block(block.clone()).unwrap();
        let retrieved = store.get_block(Height::new(42)).unwrap();
        assert_eq!(retrieved.block.header.height, Height::new(42));
    }

    #[test]
    fn missing_block_errors() {
        let store = MemoryBackend::new();
        assert!(store.get_block(Height::new(99)).is_err());
    }

    #[test]
    fn highest_block_tracks_blocks() {
        let mut store = MemoryBackend::new();
        assert_eq!(store.highest_block(), None);
        store.put_block(dummy_block(5)).unwrap();
        store.put_block(dummy_block(10)).unwrap();
        assert_eq!(store.highest_block(), Some(Height::new(10)));
    }

    #[test]
    fn put_and_get_certificate() {
        let mut store = MemoryBackend::new();
        let cert = dummy_cert();
        store
            .put_certificate(CertificateKind::Finality, cert.clone())
            .unwrap();
        let retrieved = store.get_certificate(&CertificateKind::Finality).unwrap();
        assert_eq!(retrieved.payload.height, Height::new(1));
    }

    #[test]
    fn put_and_get_state() {
        let mut store = MemoryBackend::new();
        let root = StateRoot::new([0xAA; 32]);
        let data = vec![1, 2, 3];
        store.put_state(Height::new(7), root, data.clone()).unwrap();
        let retrieved = store.get_state(Height::new(7)).unwrap();
        assert_eq!(retrieved, &[1, 2, 3]);
        assert_eq!(store.state_root(Height::new(7)), Some(root));
    }

    #[test]
    fn vote_record_survives_memory_backend_clone() {
        let engine_id = EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1));
        let vote = VoteRecord {
            validator: sage_core::ValidatorId::new(1),
            engine_id,
            view: sage_core::View::new(7),
            height: Height::new(9),
            block_hash: Hash32::new([9; 32]),
        };
        let mut store = MemoryBackend::new();
        store.put_vote(vote).unwrap();

        let restarted = store.clone();
        let recovered = restarted
            .get_vote(vote.validator, vote.engine_id, vote.view)
            .expect("vote should be persisted across restart clone");
        assert_eq!(recovered.block_hash, vote.block_hash);
        assert_eq!(recovered.height, Height::new(9));
    }

    #[test]
    fn conflicting_vote_for_same_validator_engine_view_is_rejected() {
        let engine_id = EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1));
        let key = (
            sage_core::ValidatorId::new(2),
            engine_id,
            sage_core::View::new(3),
        );
        let mut store = MemoryBackend::new();
        store
            .put_vote(VoteRecord {
                validator: key.0,
                engine_id: key.1,
                view: key.2,
                height: Height::new(4),
                block_hash: Hash32::new([4; 32]),
            })
            .unwrap();
        let err = store
            .put_vote(VoteRecord {
                validator: key.0,
                engine_id: key.1,
                view: key.2,
                height: Height::new(4),
                block_hash: Hash32::new([5; 32]),
            })
            .unwrap_err();

        assert!(matches!(err, StoreError::ConflictingVote));
        let recovered = store.get_vote(key.0, key.1, key.2).unwrap();
        assert_eq!(recovered.block_hash, Hash32::new([4; 32]));
    }
}
