//! Error type for the local node runtime.
//!
//! Extracted from `lib.rs` for maintainability. `NodeError` and `NodeResult`
//! are re-exported from the crate root (`pub use error::*` in `lib.rs`), so
//! external paths such as `sage_node::NodeError` are unchanged.

/// Error type for the local node runtime.
#[derive(Debug)]
pub enum NodeError {
    Transport(String),
    Consensus(String),
    Controller(String),
    Io(String),
    Store(String),
    Lock(String),
}

pub type NodeResult<T> = Result<T, NodeError>;

impl From<sage_network::NetworkError> for NodeError {
    fn from(e: sage_network::NetworkError) -> Self {
        NodeError::Transport(e.to_string())
    }
}

impl From<sage_consensus::ConsensusError> for NodeError {
    fn from(e: sage_consensus::ConsensusError) -> Self {
        NodeError::Consensus(e.to_string())
    }
}

impl From<sage_core::CoreError> for NodeError {
    fn from(e: sage_core::CoreError) -> Self {
        NodeError::Consensus(e.to_string())
    }
}

impl From<sage_store::StoreError> for NodeError {
    fn from(e: sage_store::StoreError) -> Self {
        NodeError::Store(e.to_string())
    }
}

impl From<serde_json::Error> for NodeError {
    fn from(e: serde_json::Error) -> Self {
        NodeError::Store(e.to_string())
    }
}
