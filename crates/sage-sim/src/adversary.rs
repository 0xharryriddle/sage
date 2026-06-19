use crate::config::AdversaryConfig;
use sage_core::{Height, ValidatorId};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionState {
    pub active: bool,
    pub side_a: BTreeSet<ValidatorId>,
    pub side_b: BTreeSet<ValidatorId>,
}

impl PartitionState {
    pub fn inactive() -> Self {
        Self {
            active: false,
            side_a: BTreeSet::new(),
            side_b: BTreeSet::new(),
        }
    }

    pub fn same_side(&self, a: ValidatorId, b: ValidatorId) -> bool {
        if !self.active {
            return true;
        }
        (self.side_a.contains(&a) && self.side_a.contains(&b))
            || (self.side_b.contains(&a) && self.side_b.contains(&b))
    }

    pub fn start(&mut self, n_validators: u32, split_count: u32) {
        self.active = true;
        self.side_a.clear();
        self.side_b.clear();
        let split = split_count.min(n_validators);
        for id in 0..n_validators {
            let vid = ValidatorId::new(id);
            if id < split {
                self.side_a.insert(vid);
            } else {
                self.side_b.insert(vid);
            }
        }
    }

    pub fn end(&mut self) {
        self.active = false;
        self.side_a.clear();
        self.side_b.clear();
    }
}

#[derive(Debug, Clone)]
pub struct AdversaryModel {
    pub cfg: AdversaryConfig,
    pub partition: PartitionState,
}

impl AdversaryModel {
    pub fn new(cfg: AdversaryConfig) -> Self {
        Self {
            cfg,
            partition: PartitionState::inactive(),
        }
    }

    pub fn on_height(&mut self, height: Height, n_validators: u32) -> AdversaryAction {
        let h = height.get();
        if self.cfg.partition_enabled
            && h >= self.cfg.partition_start_height
            && h < self.cfg.partition_start_height + self.cfg.partition_duration_blocks
        {
            if !self.partition.active {
                self.partition.start(n_validators, self.cfg.partition_split);
                return AdversaryAction::StartPartition;
            }
        } else if self.partition.active {
            self.partition.end();
            return AdversaryAction::EndPartition;
        }
        AdversaryAction::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdversaryAction {
    None,
    StartPartition,
    EndPartition,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_blocks_cross_group_messages() {
        let mut partition = PartitionState::inactive();
        partition.start(4, 2);
        assert!(partition.same_side(ValidatorId::new(0), ValidatorId::new(1)));
        assert!(!partition.same_side(ValidatorId::new(0), ValidatorId::new(2)));
    }
}
