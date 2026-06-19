use sage_core::Height;

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
}
