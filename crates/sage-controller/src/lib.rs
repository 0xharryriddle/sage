//! SAGE migration controller state machine.

pub mod controller;
pub mod error;
pub mod invariants;
pub mod phase;
pub mod readiness;
pub mod rollback;
pub mod schedule;

pub use controller::SageController;
pub use error::{ControllerError, ControllerResult};
pub use invariants::{
    check_all, check_cutcert_unique, check_no_absolute_reversion, check_no_conflicting_commits,
    check_shadow_never_finalizes, check_single_finalizer, CommittedBlockSummary, CutoverSummary,
    InvariantCheck, InvariantReport, InvariantStatus, RollbackSummary, ShadowSummary,
    SystemSnapshot, ValidatorSnapshot,
};
pub use phase::MigrationPhase;
pub use readiness::{ReadinessContext, ReadinessTracker, ShadowRecord};
pub use rollback::{AbortTrigger, RollbackAnchor, RollbackManager, RollbackOutcome};
pub use schedule::Schedule;
