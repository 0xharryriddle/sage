use crate::{ControllerError, ControllerResult};
use sage_core::{ChainId, ConfigId, EngineId, Epoch, Height, StateRoot, ValidatorId};
use sage_manifest::{CertificateKind, CertificatePayload};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowRecord {
    pub height: Height,
    pub validator: ValidatorId,
    pub verdict_root: Option<StateRoot>,
    pub canonical_root: StateRoot,
    pub valid: bool,
}

#[derive(Debug, Clone)]
pub struct ReadinessContext {
    pub chain_id: ChainId,
    pub epoch: Epoch,
    pub config_id: ConfigId,
    pub target_engine: EngineId,
}

#[derive(Debug, Clone)]
pub struct ReadinessTracker {
    kappa: u64,
    streak: u64,
    window: VecDeque<ShadowRecord>,
}

impl ReadinessTracker {
    pub fn new(kappa: u64) -> Self {
        Self {
            kappa,
            streak: 0,
            window: VecDeque::new(),
        }
    }
    pub fn streak(&self) -> u64 {
        self.streak
    }
    pub fn ready(&self) -> bool {
        self.streak >= self.kappa
    }

    pub fn observe(&mut self, record: ShadowRecord) {
        let root_matches = record.verdict_root == Some(record.canonical_root);
        self.streak = if record.valid && root_matches {
            self.streak.saturating_add(1)
        } else {
            0
        };
        self.window.push_back(record);
        while self.window.len() > self.kappa as usize {
            self.window.pop_front();
        }
    }

    pub fn make_attestation_payload(
        &self,
        ctx: ReadinessContext,
    ) -> ControllerResult<CertificatePayload> {
        if !self.ready() {
            return Err(ControllerError::NotReady);
        }
        let last = self.window.back().ok_or(ControllerError::NotReady)?;
        Ok(CertificatePayload {
            chain_id: ctx.chain_id,
            epoch: ctx.epoch,
            config_id: ctx.config_id,
            kind: CertificateKind::Readiness,
            height: last.height,
            root: last.canonical_root,
            block_hash: None,
            engine_id: ctx.target_engine,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_core::{EngineGeneration, EngineKind, Hash32};

    fn rec(height: u64, valid: bool) -> ShadowRecord {
        ShadowRecord {
            height: Height::new(height),
            validator: ValidatorId::new(0),
            verdict_root: Some(Hash32::new([height as u8; 32])),
            canonical_root: Hash32::new([height as u8; 32]),
            valid,
        }
    }

    #[test]
    fn invalid_record_resets_streak() {
        let mut tracker = ReadinessTracker::new(2);
        tracker.observe(rec(1, true));
        tracker.observe(rec(2, false));
        assert_eq!(tracker.streak(), 0);
        assert!(!tracker.ready());
    }

    #[test]
    fn emits_payload_after_kappa() {
        let mut tracker = ReadinessTracker::new(2);
        tracker.observe(rec(1, true));
        tracker.observe(rec(2, true));
        let payload = tracker
            .make_attestation_payload(ReadinessContext {
                chain_id: ChainId::new("sage"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                target_engine: EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1)),
            })
            .unwrap();
        assert_eq!(payload.height, Height::new(2));
    }
}
