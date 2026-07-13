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

pub const OVERHEAD_COLUMNS: &[&str] = &[
    "experiment",
    "mode",
    "strategy",
    "seed",
    "n",
    "f",
    "txs_per_block",
    "offered_tps_model",
    "committed_tps_wall",
    "wall_seconds",
    "cpu_user_pct",
    "cpu_system_pct",
    "cpu_total_pct",
    "rss_mb_end",
    "finalized_blocks",
    "max_finalized_height",
    "committed_txs",
    "shadow_validation_count_model",
    "shadow_validation_ns_total_proxy",
    "shadow_validation_ns_per_tx_proxy",
    "cutover_height",
    "migration_success",
    "safety_violation",
    "generator_saturated",
    "run_status",
];

pub const OVERHEAD_SUMMARY_COLUMNS: &[&str] = &[
    "txs_per_block",
    "trials",
    "baseline_tps_mean",
    "sage_tps_mean",
    "tps_overhead_pct",
    "baseline_cpu_mean",
    "sage_cpu_mean",
    "cpu_overhead_pct",
    "baseline_rss_mb_mean",
    "sage_rss_mb_mean",
    "shadow_validation_count_mean",
];

pub const WAN_CUTOVER_COLUMNS: &[&str] = &[
    "profile",
    "delay_ms",
    "jitter_ms",
    "loss_pct",
    "strategy",
    "n",
    "f",
    "max_height",
    "cutover_height",
    "migration_success",
    "observed_fork",
    "total_messages",
    "run_status",
    "skip_reason",
];

pub const TESTBED_SMOKE_COLUMNS: &[&str] = &[
    "strategy",
    "n",
    "f",
    "validator",
    "migration_success",
    "max_finalized_height",
    "cutover_height",
    "observed_fork",
    "run_max_height",
    "total_duration_secs",
    "messages_sent",
    "total_messages",
    "replay_context_root",
    "replay_context_root_agreement",
    "replay_context_ok",
    "manifest_payload_hash",
    "manifest_payload_hash_agreement",
    "manifest_payload_ok",
];

pub const M4_MESSAGE_COMPLEXITY_COLUMNS: &[&str] = &[
    "n",
    "f",
    "max_height",
    "trial",
    "migration_success",
    "observed_fork",
    "total_messages",
    "per_round",
    "replay_context_ok",
    "manifest_payload_ok",
];

pub const M5_BYZANTINE_EQUIVOCATION_COLUMNS: &[&str] = &[
    "run",
    "n",
    "f",
    "h_c",
    "byzantine_id",
    "observed_fork",
    "migration_success",
    "total_messages",
    "replay_context_ok",
    "manifest_payload_ok",
];

// Required contract for the m6 SOTA fork-differential artifact. These six columns
// are the invariant contract: strategy/run/n/h_c identify the trial and
// observed_fork/migration_success carry the differential outcome the paper cites
// (Cox-inspired 10/10, hard-fork 10/10, SAGE 0/10). The orchestrator's supplementary
// quorum diagnostics (replay_context_ok, manifest_payload_ok) are OPTIONAL extras:
// regenerated runs emit them, but the authoritative committed fork-differential CSV
// does not, and they are not fabricated post-hoc. `missing_columns` allows extra
// columns, so a richer regenerated CSV still satisfies this contract.
pub const M6_SOTA_BASELINE_COLUMNS: &[&str] = &[
    "strategy",
    "run",
    "n",
    "h_c",
    "observed_fork",
    "migration_success",
];

pub const FORK_DIFFERENTIAL_COLUMNS: &[&str] = &["arm", "trial", "n", "forked", "verdict"];

pub const SCALE_LOCAL_COLUMNS: &[&str] = &[
    "profile",
    "n",
    "f",
    "trial",
    "migration_success",
    "reached_height",
    "target_height",
    "observed_fork",
    "total_messages",
];

pub const ROLLBACK_ADMISSIBILITY_COLUMNS: &[&str] = &[
    "experiment",
    "workload_class",
    "trials",
    "clean_replays",
    "fail_closed",
    "silent_misreplay",
    "expected",
    "passed",
];

pub const CERT_TIMING_COLUMNS: &[&str] = &[
    "experiment",
    "n",
    "f",
    "signers",
    "sign_total_us",
    "verify_all_p50_us",
    "verify_all_p95_us",
    "cert_bytes",
    "gather_model_us",
    "measurement_source",
];

pub fn missing_columns<'a>(headers: &'a [&'a str], required: &'a [&'a str]) -> Vec<&'a str> {
    required
        .iter()
        .copied()
        .filter(|required| !headers.iter().any(|header| header == required))
        .collect()
}
