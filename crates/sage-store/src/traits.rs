//! Storage traits for blocks, certificates, manifests, and state.
use crate::error::StoreResult;
use sage_core::{BlockHash, EngineId, Epoch, FinalizedBlock, Height, StateRoot, ValidatorId, View};
use sage_manifest::{Certificate, MigrationManifest};

/// Store for finalized blocks indexed by height.
pub trait BlockStore {
    fn put_block(&mut self, block: FinalizedBlock) -> StoreResult<()>;
    fn get_block(&self, height: Height) -> StoreResult<&FinalizedBlock>;
    fn block_exists(&self, height: Height) -> bool;
    fn highest_block(&self) -> Option<Height>;
}

/// Store for quorum certificates and migration certificates.
pub trait CertificateStore {
    fn put_certificate(&mut self, kind: CertificateKind, cert: Certificate) -> StoreResult<()>;
    fn get_certificate(&self, kind: &CertificateKind) -> StoreResult<&Certificate>;
}

/// Store for migration manifests indexed by epoch.
pub trait ManifestStore {
    fn put_manifest(&mut self, manifest: MigrationManifest) -> StoreResult<()>;
    fn get_manifest(&self, epoch: Epoch) -> StoreResult<&MigrationManifest>;
}

/// Store for execution state snapshots indexed by height.
pub trait StateStore {
    fn put_state(&mut self, height: Height, root: StateRoot, data: Vec<u8>) -> StoreResult<()>;
    fn get_state(&self, height: Height) -> StoreResult<&[u8]>;
    fn state_root(&self, height: Height) -> Option<StateRoot>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VoteRecord {
    pub validator: ValidatorId,
    pub engine_id: EngineId,
    pub view: View,
    pub height: Height,
    pub block_hash: BlockHash,
}

/// Store for safety-critical local votes used to prevent double votes after restart.
pub trait SafetyStore {
    fn put_vote(&mut self, vote: VoteRecord) -> StoreResult<()>;
    fn get_vote(
        &self,
        validator: ValidatorId,
        engine_id: EngineId,
        view: View,
    ) -> Option<&VoteRecord>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CertificateKind {
    Finality,
    Readiness,
    Cutover,
    Abort,
    Checkpoint,
}
