use crate::{ConsensusError, ConsensusResult, ValidatorSet};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumThreshold {
    pub required_power: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuorumPolicy {
    Majority,
    Bft { f: u64 },
    MigrationCutover { f: u64 },
    Weighted { numerator: u64, denominator: u64 },
}

impl QuorumPolicy {
    pub fn threshold(&self, validators: &ValidatorSet) -> ConsensusResult<QuorumThreshold> {
        let total = validators.total_power()?;
        let required_power = match *self {
            Self::Majority => total / 2 + 1,
            Self::Bft { f } => f
                .checked_mul(2)
                .and_then(|v| v.checked_add(1))
                .ok_or(ConsensusError::ArithmeticOverflow("bft threshold"))?,
            Self::MigrationCutover { f } => {
                total
                    .checked_sub(f)
                    .ok_or_else(|| ConsensusError::InvalidQuorumPolicy {
                        reason: "f exceeds total voting power".to_string(),
                    })?
            }
            Self::Weighted {
                numerator,
                denominator,
            } => {
                if denominator == 0 || numerator == 0 || numerator > denominator {
                    return Err(ConsensusError::InvalidQuorumPolicy {
                        reason: "invalid weighted quorum fraction".to_string(),
                    });
                }
                total
                    .checked_mul(numerator)
                    .and_then(|v| v.checked_add(denominator - 1))
                    .map(|v| v / denominator)
                    .ok_or(ConsensusError::ArithmeticOverflow("weighted threshold"))?
            }
        };
        if required_power == 0 || required_power > total {
            return Err(ConsensusError::InvalidQuorumPolicy {
                reason: format!("threshold {required_power} outside total {total}"),
            });
        }
        Ok(QuorumThreshold { required_power })
    }

    pub fn validate_intersection(
        &self,
        other: &Self,
        validators: &ValidatorSet,
        max_fault_power: u64,
    ) -> ConsensusResult<()> {
        let total = validators.total_power()?;
        let a = self.threshold(validators)?.required_power;
        let b = other.threshold(validators)?.required_power;
        let intersection = a.saturating_add(b).saturating_sub(total);
        if intersection <= max_fault_power {
            return Err(ConsensusError::InvalidQuorumPolicy {
                reason: format!(
                    "intersection {intersection} <= max faulty power {max_fault_power}"
                ),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_core::{ConfigId, Epoch};

    #[test]
    fn bft_threshold_for_twenty_validators_is_thirteen_when_f_six() {
        let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 20);
        assert_eq!(
            QuorumPolicy::Bft { f: 6 }
                .threshold(&set)
                .unwrap()
                .required_power,
            13
        );
    }

    #[test]
    fn migration_cutover_quorum_intersects_itself_above_faults() {
        let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 20);
        let policy = QuorumPolicy::MigrationCutover { f: 6 };
        assert!(policy.validate_intersection(&policy, &set, 6).is_ok());
    }
}
