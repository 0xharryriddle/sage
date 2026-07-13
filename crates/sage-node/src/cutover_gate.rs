//! Pure SAGE cutover-gate predicate — the single source of truth for the
//! quorum decision, shared by the runtime (`drive_quorum_cutover`) and the
//! TLA+ conformance harness (`tests/formal_conformance.rs`).
//!
//! Extracting this into a pure function is what makes the formal-conformance
//! check non-vacuous: the harness replays the SAME decision logic the runtime
//! uses over the bounded scenario space TLC explores, rather than a re-derived
//! copy that could silently drift from the runtime.
//!
//! # Correspondence to `formal/Sage.tla`
//! - [`gate_open`] is the Rust realization of the TLA `CutoverEnabled` guard's
//!   quorum arm: `SideHasQuorum(s) == Cardinality(SideAttesters(s)) >= Quorum`
//!   with `Quorum == N - F`.
//! - [`side_has_cutcert`] mirrors `SideHasQuorum` directly: it counts only the
//!   attesters on a validator's own partition side (a partition drops
//!   cross-side messages, exactly as TLA `SideAttesters(s)` filters by side).
//! - [`two_sides_can_switch`] is the negation of the TLA `DecisionUniqueness`
//!   invariant `~(SideHasQuorum(1) /\ SideHasQuorum(2))`: it returns whether a
//!   given partition split could let BOTH sides reach the threshold, i.e.
//!   whether the gate admits a cross-boundary fork.

/// The core gate predicate: a boundary switches iff it has collected at least
/// `threshold` (= `n - f` for SAGE) distinct attesters. This is the exact
/// decision the runtime makes at `drive_quorum_cutover` (`attesters >=
/// threshold`), lifted to a pure function so both the runtime and the formal
/// conformance harness call one implementation.
#[inline]
pub fn gate_open(distinct_attesters: u64, threshold: u64) -> bool {
    distinct_attesters >= threshold
}

/// Whether a partition side holds a CutCert: count the distinct attesters that
/// belong to `target_side` and apply the gate. `sides[i]` is validator `i`'s
/// partition side; `attested[i]` is whether validator `i` has broadcast a
/// readiness attestation. Under a partition a validator only receives
/// same-side attestations, so only same-side attesters count — the socket-layer
/// partition the multi-process testbed engages via `Transport::partition_to`,
/// and the TLA `SideAttesters(s)` filter.
pub fn side_has_cutcert(sides: &[u8], attested: &[bool], target_side: u8, threshold: u64) -> bool {
    debug_assert_eq!(sides.len(), attested.len());
    let same_side_attesters = sides
        .iter()
        .zip(attested.iter())
        .filter(|(&s, &a)| s == target_side && a)
        .count() as u64;
    gate_open(same_side_attesters, threshold)
}

/// Whether, for a given partition assignment and set of attesters, BOTH sides
/// of a binary `{1, 2}` split could form a CutCert under `threshold`. When this
/// is `true` the gate admits a cross-boundary fork (two disjoint quorums each
/// authorize a distinct boundary block); when `false`, quorum intersection
/// guarantees at most one boundary is ever certified.
///
/// For SAGE (`threshold = n - f`) this is unsatisfiable whenever `n >= 2f + 1`,
/// because two disjoint sets of size `>= n - f` require `2(n - f) <= n`, i.e.
/// `n <= 2f` — contradicting the BFT bound. For a blind control that switches
/// on local readiness (modeled as `threshold = 1`) a balanced split satisfies
/// it, which is the observed fork.
pub fn two_sides_can_switch(sides: &[u8], attested: &[bool], threshold: u64) -> bool {
    side_has_cutcert(sides, attested, 1, threshold)
        && side_has_cutcert(sides, attested, 2, threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_opens_at_threshold() {
        assert!(!gate_open(2, 3));
        assert!(gate_open(3, 3));
        assert!(gate_open(4, 3));
    }

    #[test]
    fn side_cutcert_counts_only_same_side_attesters() {
        // n=4, sides = [1,1,2,2], all attested. Threshold n-f = 3.
        let sides = [1u8, 1, 2, 2];
        let attested = [true, true, true, true];
        // Each side has only 2 attesters < 3 -> neither side holds a CutCert.
        assert!(!side_has_cutcert(&sides, &attested, 1, 3));
        assert!(!side_has_cutcert(&sides, &attested, 2, 3));
    }

    #[test]
    fn nf_gate_forbids_two_sided_switch_for_all_n4_splits() {
        // Exhaustive n=4, f=1 (threshold 3): no partition split with any
        // attestation subset lets both sides switch. This is DecisionUniqueness.
        let threshold = 3u64;
        for split in 0u8..(1 << 4) {
            let sides: Vec<u8> = (0..4)
                .map(|i| if (split >> i) & 1 == 0 { 1 } else { 2 })
                .collect();
            for att in 0u8..(1 << 4) {
                let attested: Vec<bool> = (0..4).map(|i| (att >> i) & 1 == 1).collect();
                assert!(
                    !two_sides_can_switch(&sides, &attested, threshold),
                    "n-f gate must never allow a two-sided switch (split={split:04b}, att={att:04b})"
                );
            }
        }
    }

    #[test]
    fn blind_gate_admits_two_sided_switch() {
        // Blind control (threshold 1): a balanced split with both sides
        // attesting DOES allow both to switch -> the observed fork.
        let sides = [1u8, 1, 2, 2];
        let attested = [true, true, true, true];
        assert!(two_sides_can_switch(&sides, &attested, 1));
    }
}
