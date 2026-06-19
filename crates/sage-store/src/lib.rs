//! SAGE storage abstraction — traits for block, certificate, manifest,
//! and state persistence, plus an in-memory backend for tests.
pub mod error;
pub mod memory;
pub mod traits;

pub use error::{StoreError, StoreResult};
pub use memory::MemoryBackend;
pub use traits::{
    BlockStore, CertificateKind, CertificateStore, ManifestStore, SafetyStore, StateStore,
    VoteRecord,
};
