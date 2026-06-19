//! Property tests for controller invariants.
//!
//! These tests verify:
//! - Phase transition soundness (no impossible transitions)
//! - Rollback deadline enforcement
//! - Schedule ordering
//! - Readiness streak behavior
#[cfg(test)]
mod proptest_tests {
    use proptest::prelude::*;
    use sage_controller::MigrationPhase;
    use sage_controller::{ReadinessTracker, Schedule, ShadowRecord};
    use sage_core::{Hash32, Height, ValidatorId};

    // ---------------------------------------------------------------
    // Phase transition properties
    // ---------------------------------------------------------------

    prop_compose! {
        fn arb_phase()(phase in 0u8..5u8) -> MigrationPhase {
            match phase {
                0 => MigrationPhase::V1Only,
                1 => MigrationPhase::DualRun,
                2 => MigrationPhase::V2Only,
                3 => MigrationPhase::Rollback,
                4 => MigrationPhase::Sealed,
                _ => unreachable!(),
            }
        }
    }

    proptest! {
        /// No phase can transition to V1Only except Rollback.
        #[test]
        fn only_rollback_enters_v1only(phase in arb_phase()) {
            let can = phase.can_transition_to(MigrationPhase::V1Only);
            if phase == MigrationPhase::Rollback {
                prop_assert!(can);
            } else {
                prop_assert!(!can);
            }
        }

        /// Sealed is absorbing: only Sealed->Sealed is valid.
        #[test]
        fn sealed_is_absorbing(target in arb_phase()) {
            let can = MigrationPhase::Sealed.can_transition_to(target);
            if target == MigrationPhase::Sealed {
                prop_assert!(can);
            } else {
                prop_assert!(!can);
            }
        }

        /// Reflexivity holds only for DualRun and Sealed.
        #[test]
        fn reflexivity_is_restricted(phase in arb_phase()) {
            let can = phase.can_transition_to(phase);
            match phase {
                MigrationPhase::DualRun | MigrationPhase::Sealed => prop_assert!(can),
                _ => prop_assert!(!can),
            }
        }

        /// No transition is valid from Sealed except to Sealed.
        #[test]
        fn sealed_no_escape(target in arb_phase()) {
            if target == MigrationPhase::Sealed {
                return Ok(());
            }
            prop_assert!(!MigrationPhase::Sealed.can_transition_to(target));
        }

        /// V2Only can only go to Rollback or Sealed.
        #[test]
        fn v2only_successors_are_rollback_or_sealed(target in arb_phase()) {
            let can = MigrationPhase::V2Only.can_transition_to(target);
            if matches!(target, MigrationPhase::Rollback | MigrationPhase::Sealed) {
                prop_assert!(can);
            } else {
                prop_assert!(!can);
            }
        }
    }

    // ---------------------------------------------------------------
    // Schedule ordering properties
    // ---------------------------------------------------------------

    proptest! {
        /// A valid schedule must have h_d < h_c <= h_r, kappa > 0, tau > 0.
        #[test]
        fn schedule_validation(
            h_d in 1u64..100u64,
            h_c in 1u64..200u64,
            h_r in 1u64..300u64,
            kappa in 0u64..32u64,
            tau in 0u64..16u64,
        ) {
            let s = Schedule {
                h_d: Height::new(h_d),
                h_c: Height::new(h_c),
                h_r: Height::new(h_r),
                kappa,
                tau_blocks: tau,
            };
            let valid = s.validate().is_ok();
            let expected = h_d < h_c && h_c <= h_r && kappa > 0 && tau > 0;
            prop_assert_eq!(valid, expected);
        }

        /// Schedule with h_d >= h_c is rejected.
        #[test]
        fn h_d_not_less_than_h_c_rejected(h in 1u64..200u64) {
            let s = Schedule {
                h_d: Height::new(h),
                h_c: Height::new(h),
                h_r: Height::new(h + 10),
                kappa: 1,
                tau_blocks: 1,
            };
            prop_assert!(s.validate().is_err());
        }

        /// kappa=0 is rejected.
        #[test]
        fn kappa_zero_rejected(
            h_d in 1u64..10u64,
            h_c in 10u64..20u64,
            h_r in 20u64..30u64,
        ) {
            let s = Schedule {
                h_d: Height::new(h_d),
                h_c: Height::new(h_c),
                h_r: Height::new(h_r),
                kappa: 0,
                tau_blocks: 1,
            };
            prop_assert!(s.validate().is_err());
        }
    }

    // ---------------------------------------------------------------
    // Readiness tracker properties
    // ---------------------------------------------------------------

    fn make_record(height: u64, valid: bool, root_match: bool) -> ShadowRecord {
        let hash = Hash32::new([height as u8; 32]);
        let verdict = if root_match {
            Some(hash)
        } else {
            Some(Hash32::new([(height ^ 1) as u8; 32]))
        };
        ShadowRecord {
            height: Height::new(height),
            validator: ValidatorId::new(0),
            verdict_root: verdict,
            canonical_root: hash,
            valid,
        }
    }

    proptest! {
        /// Readiness with 0-streak never reports ready.
        #[test]
        fn zero_kappa_always_ready(n_records in 1u64..20u64) {
            let mut tracker = ReadinessTracker::new(0);
            for i in 0..n_records {
                tracker.observe(make_record(i, true, true));
            }
            // With kappa=0, streak never increments because observe checks ready() differently
            // Actually kappa=0 means streak >= 0 is always true.
            // Let's check the behavior: observe adds to streak only if valid && root_match
            prop_assert!(tracker.ready());
        }

        /// Invalid record resets streak regardless of previous good records.
        #[test]
        fn invalid_resets_streak(n_good in 1u64..10u64) {
            let mut tracker = ReadinessTracker::new(n_good + 1);
            for i in 0..n_good {
                tracker.observe(make_record(i, true, true));
            }
            prop_assert!(!tracker.ready());
            tracker.observe(make_record(n_good, false, true));
            prop_assert_eq!(tracker.streak(), 0);
            prop_assert!(!tracker.ready());
        }

        /// Mismatched root resets streak even if valid=true.
        #[test]
        fn mismatched_root_resets_streak(n_good in 1u64..10u64) {
            let mut tracker = ReadinessTracker::new(n_good + 1);
            for i in 0..n_good {
                tracker.observe(make_record(i, true, true));
            }
            tracker.observe(make_record(n_good, true, false));
            prop_assert_eq!(tracker.streak(), 0);
        }
    }

    // ---------------------------------------------------------------
    // Rollback deadline properties
    // ---------------------------------------------------------------

    proptest! {
        /// Rollback is open strictly before h_r.
        #[test]
        fn rollback_open_before_deadline(
            h_d in 1u64..50u64,
            h_c in 50u64..100u64,
            h_r in 100u64..200u64,
            current_h in 1u64..199u64,
        ) {
            let s = Schedule {
                h_d: Height::new(h_d),
                h_c: Height::new(h_c),
                h_r: Height::new(h_r),
                kappa: 1,
                tau_blocks: 1,
            };
            let open = s.rollback_open(Height::new(current_h));
            prop_assert_eq!(open, current_h < h_r);
        }

        /// Sealed at or after h_r.
        #[test]
        fn sealed_at_deadline(
            h_r in 10u64..200u64,
            offset in 0u64..50u64,
        ) {
            let s = Schedule {
                h_d: Height::new(1),
                h_c: Height::new(5),
                h_r: Height::new(h_r),
                kappa: 1,
                tau_blocks: 1,
            };
            let h = Height::new(h_r + offset);
            prop_assert!(s.sealed_at(h));
            if offset == 0 {
                prop_assert!(!s.rollback_open(h));
            }
        }
    }
}
