use crate::{ControllerError, ControllerResult};
use sage_core::Height;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    pub h_d: Height,
    pub h_c: Height,
    pub h_r: Height,
    pub kappa: u64,
    pub tau_blocks: u64,
}

impl Schedule {
    pub fn validate(&self) -> ControllerResult<()> {
        if self.h_d >= self.h_c {
            return Err(ControllerError::InvalidSchedule(
                "h_d must be less than h_c".to_string(),
            ));
        }
        if self.h_c > self.h_r {
            return Err(ControllerError::InvalidSchedule(
                "h_c must be <= h_r".to_string(),
            ));
        }
        if self.kappa == 0 {
            return Err(ControllerError::InvalidSchedule(
                "kappa must be > 0 for SAGE".to_string(),
            ));
        }
        if self.tau_blocks == 0 {
            return Err(ControllerError::InvalidSchedule(
                "tau_blocks must be > 0".to_string(),
            ));
        }
        Ok(())
    }

    pub fn is_dual_start(&self, h: Height) -> bool {
        h >= self.h_d
    }
    pub fn can_cutover_at_next_height(&self, h: Height) -> bool {
        h.checked_next()
            .map(|next| next >= self.h_c)
            .unwrap_or(false)
    }
    pub fn rollback_open(&self, h: Height) -> bool {
        h < self.h_r
    }
    pub fn sealed_at(&self, h: Height) -> bool {
        h >= self.h_r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_ordered_schedule() {
        let schedule = Schedule {
            h_d: Height::new(10),
            h_c: Height::new(20),
            h_r: Height::new(30),
            kappa: 8,
            tau_blocks: 4,
        };
        assert!(schedule.validate().is_ok());
        assert!(schedule.rollback_open(Height::new(29)));
        assert!(schedule.sealed_at(Height::new(30)));
    }
}
