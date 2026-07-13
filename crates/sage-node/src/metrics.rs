//! Metrics emitted by the local node runtime.
//!
//! Extracted from `lib.rs` for maintainability. `NodeMetrics` is re-exported
//! from the crate root (`pub use metrics::NodeMetrics` in `lib.rs`), so external
//! paths such as `sage_node::NodeMetrics` are unchanged.

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct NodeMetrics {
    pub finalized_blocks: u64,
    pub migration_success: bool,
    pub safety_violation: bool,
    pub max_finalized_height: u64,
    pub total_duration_secs: f64,
}
