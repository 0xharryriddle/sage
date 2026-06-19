//! Network errors.
use sage_consensus::ConsensusError;
use sage_core::ValidatorId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("channel closed for validator {0}")]
    ChannelClosed(ValidatorId),

    #[error("unknown peer {0}")]
    UnknownPeer(ValidatorId),

    #[error("send error: {0}")]
    SendError(String),

    #[error("consensus error: {0}")]
    Consensus(#[from] ConsensusError),

    #[error("io error: {0}")]
    Io(String),

    #[error("serialization error: {0}")]
    Serde(String),
}

pub type NetworkResult<T> = Result<T, NetworkError>;
