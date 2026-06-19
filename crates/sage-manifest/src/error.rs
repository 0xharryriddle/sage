use sage_core::{ChainId, ConfigId, EngineId, Epoch, StateRoot, ValidatorId};

pub type ManifestResult<T> = Result<T, ManifestError>;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("insufficient quorum: required {required}, actual {actual}")]
    InsufficientQuorum { required: u64, actual: u64 },
    #[error("unknown signer {signer}")]
    UnknownSigner { signer: ValidatorId },
    #[error("signature payload mismatch")]
    PayloadMismatch,
    #[error("wrong chain id: expected {expected}, actual {actual}")]
    WrongChainId { expected: ChainId, actual: ChainId },
    #[error("wrong epoch: expected {expected}, actual {actual}")]
    WrongEpoch { expected: Epoch, actual: Epoch },
    #[error("wrong config id: expected {expected}, actual {actual}")]
    WrongConfigId {
        expected: ConfigId,
        actual: ConfigId,
    },
    #[error("wrong boundary root: expected {expected}, actual {actual}")]
    WrongBoundaryRoot {
        expected: StateRoot,
        actual: StateRoot,
    },
    #[error("wrong engine id: expected {expected:?}, actual {actual:?}")]
    WrongEngine {
        expected: EngineId,
        actual: EngineId,
    },
    #[error("manifest parent is not legacy-finalized")]
    ManifestNotLegacyFinalized,
    #[error("invalid signature")]
    SignatureInvalid,
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
}
