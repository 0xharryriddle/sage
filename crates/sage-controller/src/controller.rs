use crate::{ReadinessContext, ReadinessTracker, Schedule, ShadowRecord};
use sage_core::Height;
use sage_manifest::CertificatePayload;

#[derive(Debug, Clone)]
pub struct SageController {
    pub schedule: Schedule,
    pub readiness: ReadinessTracker,
    pub cutover_payload: Option<CertificatePayload>,
}

impl SageController {
    pub fn new(schedule: Schedule) -> Self {
        Self {
            readiness: ReadinessTracker::new(schedule.kappa),
            schedule,
            cutover_payload: None,
        }
    }

    pub fn observe_shadow(
        &mut self,
        record: ShadowRecord,
        ctx: ReadinessContext,
    ) -> Option<Height> {
        let height = record.height;
        self.readiness.observe(record);
        if self.cutover_payload.is_none()
            && self.readiness.ready()
            && self.schedule.can_cutover_at_next_height(height)
        {
            if let Ok(payload) = self.readiness.make_attestation_payload(ctx) {
                self.cutover_payload = Some(payload);
                return height.checked_next();
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_core::{
        ChainId, ConfigId, EngineGeneration, EngineId, EngineKind, Epoch, Hash32, ValidatorId,
    };

    #[test]
    fn emits_cutover_height_after_readiness() {
        let schedule = Schedule {
            h_d: Height::new(1),
            h_c: Height::new(3),
            h_r: Height::new(5),
            kappa: 2,
            tau_blocks: 4,
        };
        let mut controller = SageController::new(schedule);
        let ctx = ReadinessContext {
            chain_id: ChainId::new("sage"),
            epoch: Epoch::new(1),
            config_id: ConfigId::new(1),
            target_engine: EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1)),
        };
        let rec = |h| ShadowRecord {
            height: Height::new(h),
            validator: ValidatorId::new(0),
            verdict_root: Some(Hash32::new([h as u8; 32])),
            canonical_root: Hash32::new([h as u8; 32]),
            valid: true,
        };
        assert_eq!(controller.observe_shadow(rec(1), ctx.clone()), None);
        assert_eq!(controller.observe_shadow(rec(2), ctx), Some(Height::new(3)));
    }
}
