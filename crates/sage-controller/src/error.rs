use sage_core::{Hash32, Height};

pub type ControllerResult<T> = Result<T, ControllerError>;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ControllerError {
    #[error("invalid schedule: {0}")]
    InvalidSchedule(String),
    #[error("invalid phase transition")]
    InvalidPhaseTransition,
    #[error("readiness threshold has not been reached")]
    NotReady,
    #[error("missing rollback anchor")]
    MissingRollbackAnchor,
    #[error("rollback deadline passed at height {height}")]
    RollbackDeadlinePassed { height: Height },
    #[error("missing replay context root for {blocks} provisional block contexts")]
    MissingReplayContextRoot { blocks: usize },
    #[error("replay context root mismatch: expected {expected}, actual {actual}")]
    ReplayContextRootMismatch { expected: Hash32, actual: Hash32 },
    #[error("replay context incomplete: expected {expected} block contexts, actual {actual}")]
    ReplayContextIncomplete { expected: usize, actual: usize },
    #[error("replay context height mismatch: expected {expected}, actual {actual}")]
    ReplayContextHeightMismatch { expected: Height, actual: Height },
}
