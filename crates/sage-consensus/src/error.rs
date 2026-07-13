use sage_core::{BlockHash, Height, ValidatorId, View};

pub type ConsensusResult<T> = Result<T, ConsensusError>;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConsensusError {
    #[error("invalid quorum policy: {reason}")]
    InvalidQuorumPolicy { reason: String },

    #[error("insufficient voting power: required {required}, actual {actual}")]
    InsufficientVotes { required: u64, actual: u64 },

    #[error("unknown validator {validator}")]
    UnknownValidator { validator: ValidatorId },

    #[error("arithmetic overflow while computing {0}")]
    ArithmeticOverflow(&'static str),

    #[error("stale view: current {current}, actual {actual}")]
    StaleView { current: View, actual: View },

    #[error("double vote attempt by {validator} in view {view}")]
    DoubleVoteAttempt { validator: ValidatorId, view: View },

    #[error("invalid proposal at height {height}: {reason}")]
    InvalidProposal { height: Height, reason: String },

    #[error("block {block_hash} is not known")]
    UnknownBlock { block_hash: BlockHash },

    #[error("invalid certificate: {reason}")]
    InvalidCertificate { reason: String },
}
