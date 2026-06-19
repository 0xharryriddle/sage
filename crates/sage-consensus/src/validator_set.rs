use crate::{ConsensusError, ConsensusResult};
use sage_core::{ConfigId, Epoch, ValidatorId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorInfo {
    pub id: ValidatorId,
    pub voting_power: u64,
    pub active_from: Epoch,
    pub active_until: Option<Epoch>,
}

impl ValidatorInfo {
    pub fn active_in(&self, epoch: Epoch) -> bool {
        self.active_from <= epoch && self.active_until.map(|end| epoch < end).unwrap_or(true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorSet {
    pub config_id: ConfigId,
    validators: BTreeMap<ValidatorId, ValidatorInfo>,
}

impl ValidatorSet {
    pub fn new(config_id: ConfigId, validators: impl IntoIterator<Item = ValidatorInfo>) -> Self {
        let validators = validators.into_iter().map(|info| (info.id, info)).collect();
        Self {
            config_id,
            validators,
        }
    }

    pub fn equal_power(config_id: ConfigId, epoch: Epoch, n: u32) -> Self {
        Self::new(
            config_id,
            (0..n).map(|id| ValidatorInfo {
                id: ValidatorId::new(id),
                voting_power: 1,
                active_from: epoch,
                active_until: None,
            }),
        )
    }

    pub fn contains(&self, id: ValidatorId) -> bool {
        self.validators.contains_key(&id)
    }

    pub fn power_of(&self, id: ValidatorId) -> Option<u64> {
        self.validators.get(&id).map(|v| v.voting_power)
    }

    pub fn total_power(&self) -> ConsensusResult<u64> {
        self.validators.values().try_fold(0u64, |acc, info| {
            acc.checked_add(info.voting_power)
                .ok_or(ConsensusError::ArithmeticOverflow("validator total power"))
        })
    }

    pub fn power_of_signers<'a>(
        &self,
        signers: impl IntoIterator<Item = &'a ValidatorId>,
    ) -> ConsensusResult<u64> {
        signers.into_iter().try_fold(0u64, |acc, id| {
            let power = self
                .power_of(*id)
                .ok_or(ConsensusError::UnknownValidator { validator: *id })?;
            acc.checked_add(power)
                .ok_or(ConsensusError::ArithmeticOverflow("signer voting power"))
        })
    }

    pub fn active_ids(&self) -> BTreeSet<ValidatorId> {
        self.validators.keys().copied().collect()
    }
    pub fn len(&self) -> usize {
        self.validators.len()
    }
    pub fn is_empty(&self) -> bool {
        self.validators.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_power_set_counts_total_power() {
        let set = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 4);
        assert_eq!(set.total_power().unwrap(), 4);
        assert!(set.contains(ValidatorId::new(3)));
    }
}
