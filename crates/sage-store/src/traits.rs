//! Storage traits for blocks, certificates, manifests, and state.
use crate::error::StoreResult;
use sage_core::{BlockHash, EngineId, Epoch, FinalizedBlock, Height, StateRoot, ValidatorId, View};
use sage_manifest::{Certificate, CutoverCertificate, MigrationManifest};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationDecisionRecord {
    pub version: u32,
    pub required_power: u64,
    pub cut_cert: CutoverCertificate,
}

/// Store for the authority decision that must survive a validator restart.
pub trait MigrationStore {
    fn put_migration_decision(&mut self, record: MigrationDecisionRecord) -> StoreResult<()>;
    fn migration_decision(&self) -> Option<&MigrationDecisionRecord>;
}

/// One authority-visible finalized transition. Implementations publish all fields or none.
#[derive(Debug, Clone)]
pub struct CommittedTransition {
    pub block: FinalizedBlock,
    pub state_root: StateRoot,
    pub state: Vec<u8>,
    pub migration_decision: Option<MigrationDecisionRecord>,
}

pub trait TransactionalStore {
    fn commit_transition(&mut self, transition: CommittedTransition) -> StoreResult<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CertificateKind {
    Finality,
    Readiness,
    Cutover,
    Abort,
    Checkpoint,
}
