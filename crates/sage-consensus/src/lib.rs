//! Consensus abstractions and the reference target engine.
//!
//! The `ConsensusEngine` trait (`engine`) is the integration surface SAGE
//! migrates across; `poa` and `hotstuff` are the legacy source and reference
//! target instantiations. The engines are deliberately interchangeable behind
//! the trait — SAGE's contribution is the safe boundary crossing, not a
//! specific engine — so a production engine can replace `hotstuff` without
//! touching the controller or the proofs.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod engine;
pub mod error;
pub mod hotstuff;
pub mod hotstuff_shadow;
pub mod message;
pub mod poa;
pub mod quorum;
pub mod raft;
pub mod validator_set;

pub use engine::{
    BootstrapAnchor, ConsensusEngine, FinalizationCertificate, ProposeContext, ShadowVerdict,
};
pub use error::{ConsensusError, ConsensusResult};
pub use hotstuff::{HotStuffEngine, HotStuffMessage, QuorumCertificate, TimeoutCertificate};
pub use hotstuff_shadow::ShadowValidator;
pub use message::{ConsensusMessage, MessageEnvelope, SageMessage};
pub use poa::{PoaEngine, PoaMessage};
pub use quorum::{QuorumPolicy, QuorumThreshold};
pub use raft::{RaftEngine, RaftMessage};
pub use validator_set::{ValidatorInfo, ValidatorSet};
