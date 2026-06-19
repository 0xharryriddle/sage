use crate::{ControllerError, ControllerResult, Schedule};
use sage_core::{BlockHash, FinalizedBlock, Height, StateRoot, Transaction};
use sage_manifest::Certificate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollbackAnchor {
    pub boundary_height: Height,
    pub boundary_root: StateRoot,
    pub boundary_hash: BlockHash,
    pub legacy_certificate: Certificate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum AbortTrigger {
    ProgressTimeout,
    AbortCert(Certificate),
    ManifestMismatch,
    OperatorFailClosed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollbackOutcome {
    pub restored_height: Height,
    pub restored_root: StateRoot,
    pub replay_transactions: Vec<Transaction>,
    pub discarded_blocks: usize,
}

#[derive(Debug, Clone, Default)]
pub struct RollbackManager {
    anchor: Option<RollbackAnchor>,
    provisional: Vec<FinalizedBlock>,
}

impl RollbackManager {
    pub fn install_anchor(&mut self, anchor: RollbackAnchor) {
        self.anchor = Some(anchor);
    }
    pub fn record_provisional(&mut self, block: FinalizedBlock) {
        self.provisional.push(block);
    }

    pub fn rollback(
        &mut self,
        _trigger: AbortTrigger,
        at: Height,
        schedule: &Schedule,
    ) -> ControllerResult<RollbackOutcome> {
        if !schedule.rollback_open(at) {
            return Err(ControllerError::RollbackDeadlinePassed { height: at });
        }
        let anchor = self
            .anchor
            .clone()
            .ok_or(ControllerError::MissingRollbackAnchor)?;
        let replay_transactions = self
            .provisional
            .iter()
            .flat_map(|b| b.block.txs.clone())
            .collect();
        let discarded_blocks = self.provisional.len();
        self.provisional.clear();
        Ok(RollbackOutcome {
            restored_height: anchor.boundary_height,
            restored_root: anchor.boundary_root,
            replay_transactions,
            discarded_blocks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_rollback_at_deadline() {
        let mut manager = RollbackManager::default();
        let schedule = Schedule {
            h_d: Height::new(10),
            h_c: Height::new(20),
            h_r: Height::new(30),
            kappa: 8,
            tau_blocks: 4,
        };
        assert!(matches!(
            manager.rollback(AbortTrigger::ProgressTimeout, Height::new(30), &schedule),
            Err(ControllerError::RollbackDeadlinePassed { .. })
        ));
    }
}
