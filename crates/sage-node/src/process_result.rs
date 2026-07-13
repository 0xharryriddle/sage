//! Serializable result of a single-validator process run.
//!
//! Extracted from `lib.rs` for maintainability. `ProcessResult` is re-exported
//! from the crate root (`pub use process_result::ProcessResult` in `lib.rs`),
//! so external paths such as `sage_node::ProcessResult` are unchanged. The
//! methods that build one live on `ProcessValidator` in `lib.rs`; this module
//! holds only the data shape.

/// Result of a single-validator process run, serializable so an orchestrator
/// can parse one JSON line per process and check cross-process agreement.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ProcessResult {
    pub validator: u32,
    pub migration_success: bool,
    pub cutover_height: Option<u64>,
    pub max_finalized_height: u64,
    pub total_duration_secs: f64,
    /// (height, block-hash hex) for every committed block, so the orchestrator
    /// can detect an OBSERVED cross-process fork: two processes reporting
    /// different hashes at the same height.
    pub committed: Vec<(u64, String)>,
    /// Hex of the ed25519-signed cutover certificate payload, present only when
    /// this node sealed the migration with real crypto. Empty otherwise.
    pub cutover_cert: Option<String>,
    /// Hex of the cutover certificate payload hash this node signed. The
    /// orchestrator re-derives each validator's verifying key from its id+seed
    /// and verifies `cutover_cert` against this hash, then counts distinct
    /// valid signers toward the 2f+1 CutCert threshold.
    pub cutover_payload_hash: Option<String>,
    /// Workload seed this node ran with — the orchestrator needs it to
    /// re-derive deterministic verifying keys for signature verification.
    pub seed: u64,
    /// Total consensus messages this validator actually wrote to the wire over
    /// the whole run (real socket sends, excluding partition-blocked and
    /// impairment-dropped). The orchestrator sums these across validators to
    /// plot empirical per-round message complexity against n (expected O(n^2)
    /// for an all-to-all BFT broadcast pattern).
    pub messages_sent: u64,
    /// Per-committed-block diagnostic detail: (height, state_root_hex,
    /// engine_kind) for every committed block. Lets an investigator decide
    /// whether a height where two validators report different BLOCK HASHES is a
    /// genuine state-level safety violation (different state_root) or merely a
    /// boundary-encoding artifact: the block hash binds `engine_id`
    /// (block.rs encode_canonical), so the SAME logical block (identical txs,
    /// identical state_root, identical parent) finalized under PoA on one node
    /// and under HotStuff on another at the cutover boundary hashes differently
    /// while representing the same state. Empty unless populated by the run.
    #[serde(default)]
    pub committed_detail: Vec<(u64, String, String)>,
    /// Replay-context root for finalized reversible-window blocks. This is a
    /// testbed-level bridge toward production EVM replay support: fields not
    /// modeled by the current toy executor are explicit zero/default values,
    /// while chain id and height are captured from the finalized blocks.
    #[serde(default)]
    pub replay_context_root: Option<String>,
    /// Manifest signing payload hash for this validator's cutover view with the
    /// replay-context root included. This lets the testbed surface the exact
    /// digest a production migration manifest would sign.
    #[serde(default)]
    pub manifest_payload_hash: Option<String>,
    /// Total transactions finalized across all committed blocks. Basis for the
    /// committed-throughput measurement (real wall-clock TPS on real hosts).
    #[serde(default)]
    pub total_txs: u64,
    /// Committed throughput in transactions per second over the whole run
    /// (total_txs / total_duration_secs). Measured on real hosts, not modeled.
    #[serde(default)]
    pub committed_tps: f64,
    /// Inter-commit finalization-gap percentiles in MICROSECONDS, measured at
    /// pacemaker-tick resolution (the gap between consecutive finalization
    /// timestamps). p50/p95/p99/max. This is the empirical realization of the
    /// T_bdy liveness bound (paper Thm 3): the max gap should include the
    /// one-time cutover handoff perturbation, the tail should not diverge.
    #[serde(default)]
    pub inter_commit_p50_micros: u64,
    #[serde(default)]
    pub inter_commit_p95_micros: u64,
    #[serde(default)]
    pub inter_commit_p99_micros: u64,
    #[serde(default)]
    pub inter_commit_max_micros: u64,
    /// The single inter-commit gap that straddles the cutover height
    /// (h_c-1 -> h_c), i.e. the observed handoff perturbation in microseconds.
    /// Zero if this node never observed both boundary heights.
    #[serde(default)]
    pub cutover_gap_micros: u64,
}
