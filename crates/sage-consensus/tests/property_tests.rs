//! Property tests for consensus crate: quorum thresholds and validator sets.
#[cfg(test)]
mod proptest_tests {
    use proptest::prelude::*;
    use sage_consensus::{QuorumPolicy, ValidatorSet};
    use sage_core::{ConfigId, Epoch, ValidatorId};
    use std::collections::BTreeSet;

    // ---------------------------------------------------------------
    // Validator set properties
    // ---------------------------------------------------------------

    proptest! {
        /// Equal-power set has total_power == n.
        #[test]
        fn equal_power_total_is_n(n in 1u32..100u32) {
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            prop_assert_eq!(set.total_power().unwrap(), n as u64);
            prop_assert_eq!(set.len(), n as usize);
        }

        /// All validators in equal-power set are present.
        #[test]
        fn equal_power_contains_all_ids(n in 1u32..100u32) {
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            for i in 0..n {
                prop_assert!(set.contains(ValidatorId::new(i)));
            }
            prop_assert!(!set.contains(ValidatorId::new(n)));
        }

        /// Power of any single validator is 1 in equal-power set.
        #[test]
        fn equal_power_each_is_one(n in 2u32..50u32) {
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            for i in 0..n {
                prop_assert_eq!(set.power_of(ValidatorId::new(i)), Some(1));
            }
        }
    }

    // ---------------------------------------------------------------
    // Quorum threshold properties
    // ---------------------------------------------------------------

    proptest! {
        /// BFT threshold is exactly 2f + 1.
        #[test]
        fn bft_threshold_is_2f_plus_1(n in 4u32..100u32, f_frac in 1u32..30u32) {
            let f = (n as u64 * f_frac as u64 / 100u64 + 1).max(1).min(n as u64 / 3);
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            let policy = QuorumPolicy::Bft { f };
            let thresh = policy.threshold(&set).unwrap();
            prop_assert_eq!(thresh.required_power, 2 * f + 1);
            // BFT threshold must be <= total power
            prop_assert!(thresh.required_power <= set.total_power().unwrap());
        }

        /// BFT threshold must not exceed n (total voting power).
        #[test]
        fn bft_threshold_within_total(n in 4u32..100u32) {
            let f = n as u64 / 3;
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            let policy = QuorumPolicy::Bft { f };
            let thresh = policy.threshold(&set).unwrap();
            prop_assert!(thresh.required_power <= n as u64);
        }

        /// Migration cutover threshold is n - f.
        #[test]
        fn migration_cutover_is_n_minus_f(n in 4u32..100u32, f_frac in 1u32..33u32) {
            let f = (n as u64 * f_frac as u64 / 100u64 + 1).max(1).min(n as u64 / 3);
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            let policy = QuorumPolicy::MigrationCutover { f };
            let thresh = policy.threshold(&set).unwrap();
            prop_assert_eq!(thresh.required_power, n as u64 - f);
            prop_assert!(thresh.required_power > 0);
        }

        /// Majority threshold is n/2 + 1.
        #[test]
        fn majority_is_n_div_2_plus_1(n in 2u32..100u32) {
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            let thresh = QuorumPolicy::Majority.threshold(&set).unwrap();
            prop_assert_eq!(thresh.required_power, n as u64 / 2 + 1);
        }

        /// BFT quorum intersection: when n >= 3f+1, two BFT quorums intersect
        /// in at least f. When intersection > f, validation passes.
        #[test]
        fn bft_quorum_intersection_above_faults(n in 4u32..50u32) {
            let f = n as u64 / 3;
            if f == 0 { return Ok(()); }
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            let policy = QuorumPolicy::Bft { f };
            let total = set.total_power().unwrap();
            let threshold = policy.threshold(&set).unwrap().required_power;
            let intersection = threshold * 2 - total;
            // Validation passes when intersection > max_fault_power
            // With max_fault_power = f - 1, this should pass when intersection >= f
            if intersection > f {
                prop_assert!(policy.validate_intersection(&policy, &set, f).is_ok());
            }
            // With max_fault_power = f - 1, intersection >= f should always pass
            if intersection >= f && f > 0 {
                prop_assert!(policy.validate_intersection(&policy, &set, f - 1).is_ok());
            }
        }

        /// Quorum intersection with insufficient intersection fails.
        #[test]
        fn insufficient_intersection_fails(n in 4u32..50u32) {
            let f = n as u64 / 3;
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            let policy = QuorumPolicy::Bft { f };
            // With max_fault_power > intersection, validation should fail
            // BFT quorum intersection = (2f+1)*2 - n = 4f+2 - (3f+1) = f+1
            // So max_fault_power = f is OK, but f+1 is not
            let total = set.total_power().unwrap();
            let threshold = policy.threshold(&set).unwrap().required_power;
            let intersection = threshold * 2 - total;
            // max_fault_power > intersection should fail
            let too_high = intersection + 1;
            let result = policy.validate_intersection(&policy, &set, too_high);
            if too_high < total {
                prop_assert!(result.is_err());
            }
        }

        /// Weighted quorum: fraction num/den applied to total.
        #[test]
        fn weighted_quorum_is_num_over_den(n in 1u32..100u32, num in 1u64..10u64, den in 5u64..20u64) {
            if num > den { return Ok(()); }
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            let policy = QuorumPolicy::Weighted { numerator: num, denominator: den };
            let thresh = policy.threshold(&set).unwrap();
            let expected = ((n as u64 * num).div_ceil(den)).max(1);
            prop_assert_eq!(thresh.required_power, expected);
        }

        /// Quorum threshold must never exceed total voting power.
        #[test]
        fn threshold_never_exceeds_total(n in 1u32..100u32) {
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            let policies = [
                QuorumPolicy::Majority,
                QuorumPolicy::Bft { f: (n as u64).min(1) / 3 + 1 },
                QuorumPolicy::MigrationCutover { f: 1 },
                QuorumPolicy::Weighted { numerator: 2, denominator: 3 },
            ];
            for policy in policies {
                if let Ok(thresh) = policy.threshold(&set) {
                    prop_assert!(thresh.required_power <= n as u64,
                        "policy {:?} threshold {} > {} total", policy, thresh.required_power, n);
                }
            }
        }
    }

    // ---------------------------------------------------------------
    // Power of signers properties
    // ---------------------------------------------------------------

    proptest! {
        /// All signers from the set: power_of_signers == total_power.
        #[test]
        fn all_signers_equals_total_power(n in 1u32..50u32) {
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            let all: BTreeSet<ValidatorId> = (0..n).map(ValidatorId::new).collect();
            prop_assert_eq!(set.power_of_signers(&all).unwrap(), n as u64);
        }

        /// Single signer has power 1 in equal-power set.
        #[test]
        fn single_signer_power_is_one(n in 1u32..50u32) {
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            let one: BTreeSet<ValidatorId> = [ValidatorId::new(0)].into_iter().collect();
            prop_assert_eq!(set.power_of_signers(&one).unwrap(), 1);
        }

        /// Unknown signer causes error.
        #[test]
        fn unknown_signer_errors(n in 1u32..50u32) {
            let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), n);
            let bad: BTreeSet<ValidatorId> = [ValidatorId::new(n)].into_iter().collect();
            prop_assert!(set.power_of_signers(&bad).is_err());
        }
    }
}
