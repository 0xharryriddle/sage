//! Storage errors for the sage-store crate.
use sage_core::Height;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("block not found at height {0}")]
    BlockNotFound(Height),

    #[error("certificate not found")]
    CertificateNotFound,

    #[error("manifest not found for epoch {0}")]
    ManifestNotFound(u64),

    #[error("state not found at height {0}")]
    StateNotFound(Height),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid durable store image: {0}")]
    InvalidImage(String),

    #[error("conflicting vote already exists for safety key")]
    ConflictingVote,

    #[error("conflicting migration authority decision already exists")]
    ConflictingMigrationDecision,

    #[error("storage error: {0}")]
    General(String),
}

pub type StoreResult<T> = Result<T, StoreError>;
