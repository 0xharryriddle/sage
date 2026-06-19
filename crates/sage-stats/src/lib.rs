//! Statistics crate for SAGE journal experiments.
//! Provides confidence intervals, hypothesis testing, and multiple comparison correction.

pub mod ci;
pub mod correction;
pub mod error;
pub mod hypothesis;

pub use ci::{mean_ci_bootstrap, wilson_interval, Interval};
pub use correction::{holm_bonferroni, AdjustedDecision};
pub use error::{StatsError, StatsResult};
pub use hypothesis::{mann_whitney_u, MannWhitneyResult};

pub fn crate_ready() -> bool {
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_is_ready() {
        assert!(super::crate_ready());
    }
}
