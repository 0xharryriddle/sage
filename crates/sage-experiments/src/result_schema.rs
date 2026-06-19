//! Shared CSV schema expectations for experiment outputs.
//!
//! Binaries still own their row structs, but these constants provide a single
//! place for artifact-gate checks and future schema convergence.

pub const COMMON_COLUMNS: &[&str] = &[
    "strategy",
    "seed",
    "run_status",
    "finalized_blocks",
    "safety_violation",
];

pub const RQ1_COLUMNS: &[&str] = &[
    "strategy",
    "seed",
    "run_status",
    "finalized_blocks",
    "max_finalized_height",
    "safety_violation",
    "downtime_micros",
    "finality_gap",
    "cutover_latency_micros",
];

/// SAGE-vs-baseline statistical comparison output (Mann-Whitney U +
/// rank-biserial effect size + Holm-Bonferroni family-wise decision).
pub const RQ1_STATS_COLUMNS: &[&str] = &[
    "metric",
    "baseline",
    "sage_mean",
    "baseline_mean",
    "u_statistic",
    "p_value",
    "rank_biserial",
    "holm_adjusted_alpha",
    "reject_null",
    "n_sage",
    "n_baseline",
];

pub const RQ2_COLUMNS: &[&str] = &[
    "strategy",
    "partition_split",
    "partition_duration_blocks",
    "seed",
    "safety_violation",
    "migration_success",
    "finalized_blocks",
    "max_finalized_height",
    "run_status",
    "disjoint_quorum_windows",
];

pub const RQ3_COLUMNS: &[&str] = &[
    "strategy",
    "kappa",
    "seed",
    "run_status",
    "finalized_blocks",
    "migration_success",
    "safety_violation",
    "cutover_height",
    "cutover_latency_micros",
    "max_finalized_height",
];

pub const RQ4_COLUMNS: &[&str] = &[
    "abort_height",
    "run_status",
    "finalized_blocks",
    "max_finalized_height",
    "safety_violation",
    "migration_success",
    "rollback_occurred",
    "rollback_latency_micros",
    "absolute_reversion_rejected",
];

pub const SAFETY_COLUMNS: &[&str] = &[
    "strategy",
    "seed",
    "run_status",
    "finalized_blocks",
    "safety_violation",
    "migration_success",
    "max_finalized_height",
    "disjoint_quorum_windows",
];

pub fn missing_columns<'a>(headers: &'a [&'a str], required: &'a [&'a str]) -> Vec<&'a str> {
    required
        .iter()
        .copied()
        .filter(|required| !headers.iter().any(|header| header == required))
        .collect()
}
