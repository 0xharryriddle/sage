use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationPhase {
    V1Only,
    DualRun,
    V2Only,
    Rollback,
    Sealed,
}

impl MigrationPhase {
    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::V1Only, Self::DualRun)
                | (Self::DualRun, Self::DualRun)
                | (Self::DualRun, Self::V2Only)
                | (Self::DualRun, Self::Rollback)
                | (Self::V2Only, Self::Rollback)
                | (Self::V2Only, Self::Sealed)
                | (Self::Rollback, Self::V1Only)
                | (Self::Sealed, Self::Sealed)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_direct_v1_to_v2_transition() {
        assert!(!MigrationPhase::V1Only.can_transition_to(MigrationPhase::V2Only));
    }
}
