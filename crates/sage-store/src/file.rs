//! Durable snapshot backend using write-sync-rename commits.
use crate::error::{StoreError, StoreResult};
use crate::traits::{
    BlockStore, CertificateKind, CertificateStore, CommittedTransition, ManifestStore,
    MigrationDecisionRecord, MigrationStore, SafetyStore, StateStore, TransactionalStore,
    VoteRecord,
};
use sage_core::{EngineId, Epoch, FinalizedBlock, Height, StateRoot, ValidatorId, View};
use sage_manifest::{Certificate, MigrationManifest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const STORE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StateSnapshot {
    root: StateRoot,
    data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoreImage {
    version: u32,
    blocks: BTreeMap<Height, FinalizedBlock>,
    certificates: BTreeMap<CertificateKind, Certificate>,
    manifests: BTreeMap<Epoch, MigrationManifest>,
    states: BTreeMap<Height, StateSnapshot>,
    votes: BTreeMap<(ValidatorId, EngineId, View), VoteRecord>,
    migration_decision: Option<MigrationDecisionRecord>,
}

impl Default for StoreImage {
    fn default() -> Self {
        Self {
            version: STORE_VERSION,
            blocks: BTreeMap::new(),
            certificates: BTreeMap::new(),
            manifests: BTreeMap::new(),
            states: BTreeMap::new(),
            votes: BTreeMap::new(),
            migration_decision: None,
        }
    }
}

#[derive(Debug)]
pub struct FileBackend {
    path: PathBuf,
    image: StoreImage,
}

impl FileBackend {
    pub fn open(path: impl AsRef<Path>) -> StoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        let image = if path.exists() {
            let bytes = fs::read(&path)?;
            let image: StoreImage = serde_json::from_slice(&bytes)
                .map_err(|err| StoreError::InvalidImage(err.to_string()))?;
            if image.version != STORE_VERSION {
                return Err(StoreError::InvalidImage(format!(
                    "unsupported schema version {}",
                    image.version
                )));
            }
            image
        } else {
            StoreImage::default()
        };
        Ok(Self { path, image })
    }

    fn commit_image(&self, image: &StoreImage) -> StoreResult<()> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let tmp = self.path.with_extension("tmp");
        let bytes =
            serde_json::to_vec(image).map_err(|err| StoreError::InvalidImage(err.to_string()))?;
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, &self.path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    }

    fn transact(
        &mut self,
        mutate: impl FnOnce(&mut StoreImage) -> StoreResult<()>,
    ) -> StoreResult<()> {
        let mut next = self.image.clone();
        mutate(&mut next)?;
        self.commit_image(&next)?;
        self.image = next;
        Ok(())
    }
}

impl BlockStore for FileBackend {
    fn put_block(&mut self, block: FinalizedBlock) -> StoreResult<()> {
        self.transact(move |image| {
            image.blocks.insert(block.block.header.height, block);
            Ok(())
        })
    }
    fn get_block(&self, height: Height) -> StoreResult<&FinalizedBlock> {
        self.image
            .blocks
            .get(&height)
            .ok_or(StoreError::BlockNotFound(height))
    }
    fn block_exists(&self, height: Height) -> bool {
        self.image.blocks.contains_key(&height)
    }
    fn highest_block(&self) -> Option<Height> {
        self.image.blocks.keys().last().copied()
    }
}

impl CertificateStore for FileBackend {
    fn put_certificate(&mut self, kind: CertificateKind, cert: Certificate) -> StoreResult<()> {
        self.transact(move |image| {
            image.certificates.insert(kind, cert);
            Ok(())
        })
    }
    fn get_certificate(&self, kind: &CertificateKind) -> StoreResult<&Certificate> {
        self.image
            .certificates
            .get(kind)
            .ok_or(StoreError::CertificateNotFound)
    }
}

impl ManifestStore for FileBackend {
    fn put_manifest(&mut self, manifest: MigrationManifest) -> StoreResult<()> {
        self.transact(move |image| {
            image.manifests.insert(manifest.epoch, manifest);
            Ok(())
        })
    }
    fn get_manifest(&self, epoch: Epoch) -> StoreResult<&MigrationManifest> {
        self.image
            .manifests
            .get(&epoch)
            .ok_or(StoreError::ManifestNotFound(epoch.get()))
    }
}

impl StateStore for FileBackend {
    fn put_state(&mut self, height: Height, root: StateRoot, data: Vec<u8>) -> StoreResult<()> {
        self.transact(move |image| {
            image.states.insert(height, StateSnapshot { root, data });
            Ok(())
        })
    }
    fn get_state(&self, height: Height) -> StoreResult<&[u8]> {
        self.image
            .states
            .get(&height)
            .map(|s| s.data.as_slice())
            .ok_or(StoreError::StateNotFound(height))
    }
    fn state_root(&self, height: Height) -> Option<StateRoot> {
        self.image.states.get(&height).map(|s| s.root)
    }
}

impl SafetyStore for FileBackend {
    fn put_vote(&mut self, vote: VoteRecord) -> StoreResult<()> {
        let key = (vote.validator, vote.engine_id, vote.view);
        if let Some(existing) = self.image.votes.get(&key) {
            if existing.block_hash != vote.block_hash || existing.height != vote.height {
                return Err(StoreError::ConflictingVote);
            }
            return Ok(());
        }
        self.transact(move |image| {
            image.votes.insert(key, vote);
            Ok(())
        })
    }
    fn get_vote(
        &self,
        validator: ValidatorId,
        engine_id: EngineId,
        view: View,
    ) -> Option<&VoteRecord> {
        self.image.votes.get(&(validator, engine_id, view))
    }
}

impl MigrationStore for FileBackend {
    fn put_migration_decision(&mut self, record: MigrationDecisionRecord) -> StoreResult<()> {
        if let Some(existing) = &self.image.migration_decision {
            if existing != &record {
                return Err(StoreError::ConflictingMigrationDecision);
            }
            return Ok(());
        }
        self.transact(move |image| {
            image.migration_decision = Some(record);
            Ok(())
        })
    }
    fn migration_decision(&self) -> Option<&MigrationDecisionRecord> {
        self.image.migration_decision.as_ref()
    }
}

impl TransactionalStore for FileBackend {
    fn commit_transition(&mut self, transition: CommittedTransition) -> StoreResult<()> {
        let height = transition.block.block.header.height;
        if transition.block.block.header.state_root != transition.state_root {
            return Err(StoreError::General(
                "transition state root does not match block header".into(),
            ));
        }
        self.transact(move |image| {
            image.blocks.insert(height, transition.block);
            image.states.insert(
                height,
                StateSnapshot {
                    root: transition.state_root,
                    data: transition.state,
                },
            );
            if let Some(decision) = transition.migration_decision {
                if image
                    .migration_decision
                    .as_ref()
                    .is_some_and(|old| old != &decision)
                {
                    return Err(StoreError::ConflictingMigrationDecision);
                }
                image.migration_decision = Some(decision);
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_core::{
        Block, BlockHeader, ChainId, ConfigId, EngineGeneration, EngineKind, FinalityTier, Hash32,
    };

    fn block(height: u64) -> FinalizedBlock {
        let block = Block {
            header: BlockHeader {
                chain_id: ChainId::new("test"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                height: Height::new(height),
                parent_hash: Hash32::ZERO,
                state_root: Hash32::new([height as u8; 32]),
                engine_id: EngineId::new(EngineKind::Poa, EngineGeneration::new(1)),
                finality_tier: FinalityTier::Absolute,
                manifest_hash: None,
            },
            txs: vec![],
        };
        let certificate_hash = block.hash();
        FinalizedBlock {
            block,
            certificate_hash,
        }
    }

    #[test]
    fn reopens_committed_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.json");
        let mut store = FileBackend::open(&path).unwrap();
        let b = block(1);
        store.put_block(b.clone()).unwrap();
        store
            .put_state(Height::new(1), b.block.header.state_root, vec![1, 2])
            .unwrap();
        drop(store);
        let reopened = FileBackend::open(&path).unwrap();
        assert_eq!(reopened.get_block(Height::new(1)).unwrap(), &b);
        assert_eq!(reopened.get_state(Height::new(1)).unwrap(), &[1, 2]);
    }

    #[test]
    fn corrupt_image_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.json");
        fs::write(&path, b"{truncated").unwrap();
        assert!(matches!(
            FileBackend::open(path),
            Err(StoreError::InvalidImage(_))
        ));
    }
}
