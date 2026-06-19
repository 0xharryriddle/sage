//! Core deterministic SAGE data types.
//!
//! This crate intentionally contains no networking, async runtime, disk IO, or
//! wall-clock access. It is the shared protocol model used by consensus,
//! controller, manifest verification, and the simulator.

pub mod block;
pub mod crypto;
pub mod error;
pub mod state;
pub mod time;
pub mod types;

pub use block::{Block, BlockHeader, FinalizedBlock, Transaction};
pub use error::{CoreError, CoreResult};
pub use state::{
    ChainState, ConsensusStateEnvelope, ExecutionState, MigrationSchedule, PlatformState,
};
pub use time::{DurationMicros, SimTime};
pub use types::*;
