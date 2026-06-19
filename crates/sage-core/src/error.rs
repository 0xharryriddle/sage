use crate::{BlockHash, Epoch, Height, StateRoot, ValidatorId};

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("invalid height transition: expected {expected}, got {actual}")]
    InvalidHeight { expected: Height, actual: Height },

    #[error("invalid parent hash: expected {expected}, got {actual}")]
    InvalidParent {
        expected: BlockHash,
        actual: BlockHash,
    },

    #[error("state root mismatch: expected {expected}, got {actual}")]
    StateRootMismatch {
        expected: StateRoot,
        actual: StateRoot,
    },

    #[error("unknown validator {validator}")]
    UnknownValidator { validator: ValidatorId },

    #[error("invalid epoch: expected {expected}, got {actual}")]
    InvalidEpoch { expected: Epoch, actual: Epoch },

    #[error("encoding error: {0}")]
    Encoding(String),

    #[error("arithmetic overflow while computing {0}")]
    ArithmeticOverflow(&'static str),
}
