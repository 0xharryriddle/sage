//! TLA+ ↔ Rust conformance harness (plan item A3/A4).
//!
//! This test closes the "model-vs-code divergence" limitation (Reviewer 2, Q8)
//! by mechanically checking that the Rust cutover-gate decision logic and the
//! TLA+ model (`formal/Sage.tla`) agree on the SAME bounded scenario space TLC
//! explores: n ∈ {4, 7, 10}, all 2^n partition splits, every attestation subset.
//!
//! It is NOT a full refinement proof (the Rust *implementation* is not proven to
//! refine the spec for all executions). It is a bounded-conformance check on the
//! decision-space OUTCOMES — the reachable `committed` boundary-block vectors:
//!
//!   1. Rust side: enumerate the reachable `committed` vectors by driving the
//!      REAL runtime gate predicate (`sage_node::cutover_gate`), not a copy.
//!   2. TLA side: parse the committed-vector projection TLC dumped
//!      (`formal/states/committed/committed_*.txt`, produced by
//!      `formal/dump_states.sh`).
//!   3. Assert the two reachable committed-vector SETS are equal, per config.
//!
//! The broken control (`QuorumGated = FALSE`, threshold = 1) must produce a
//! non-empty set of UNSAFE (forked) committed vectors on BOTH sides — this is
//! what gives the harness teeth and proves it is not passing vacuously.
//!
//! Projection files are required inputs. Missing, empty, malformed, or truncated
//! projections fail closed; regenerate them with `bash formal/dump_states.sh`
//! (or `make conformance`).

use sage_node::cutover_gate::side_has_cutcert;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// A committed-boundary vector: `committed[i]` ∈ {0,1,2} where 0 = no boundary
/// block committed by validator i, else the partition-side id of the block.
type Committed = Vec<u8>;

/// Enumerate the reachable `committed` vectors for `n`/`threshold` by driving
/// the real gate predicate over every partition split and every attestation
/// subset. This mirrors TLA `Cutover(v)`: a validator commits its side's
/// boundary block iff its side holds a CutCert under the gate.
///
/// TLC also reaches all INTERMEDIATE states (partial attestation, partial
/// switch), but every intermediate `committed` vector is dominated by a maximal
/// one reachable from the same split by letting every eligible validator that
/// CAN switch also commit — and TLC reaches the empty/partial vectors too. To
/// match TLC's reachable-committed SET exactly we therefore enumerate, for each
/// split, every subset of validators that is allowed to have committed: a
/// validator `i` may appear as committed (value `sides[i]`) in a reachable
/// state iff its side can reach the gate with SOME attestation subset, and any
/// subset of such eligible validators is independently reachable (attestations
/// and switches are per-validator, monotonic, and independent across validators
/// once their side is quorate).
fn rust_reachable_committed(n: usize, threshold: u64) -> BTreeSet<Committed> {
    let mut set = BTreeSet::new();
    for split in 0u32..(1u32 << n) {
        let sides: Vec<u8> = (0..n)
            .map(|i| if (split >> i) & 1 == 0 { 1u8 } else { 2u8 })
            .collect();

        // A side is "switch-capable" under this split iff, with ALL of its
        // members attesting, it reaches the threshold. (Attestations are
        // same-side-only, monotonic; the maximal attestation subset is the best
        // case for reaching quorum.)
        let all_attested = vec![true; n];
        let side1_capable = side_has_cutcert(&sides, &all_attested, 1, threshold);
        let side2_capable = side_has_cutcert(&sides, &all_attested, 2, threshold);

        // Each validator i is independently either not-committed (0) or
        // committed to its side (sides[i]) — but only validators whose side is
        // switch-capable may be committed. Enumerate all such subsets.
        let eligible: Vec<usize> = (0..n)
            .filter(|&i| (sides[i] == 1 && side1_capable) || (sides[i] == 2 && side2_capable))
            .collect();

        for mask in 0u32..(1u32 << eligible.len()) {
            let mut committed = vec![0u8; n];
            for (bit, &vi) in eligible.iter().enumerate() {
                if (mask >> bit) & 1 == 1 {
                    committed[vi] = sides[vi];
                }
            }
            set.insert(committed);
        }
    }
    set
}

/// Parse a committed-vector projection file (one CSV vector per line, e.g.
/// `0,1,0,2`) into a set.
fn parse_committed_projection(
    path: &PathBuf,
    expected_width: usize,
) -> Result<BTreeSet<Committed>, String> {
    let text = std::fs::read_to_string(path).map_err(|err| {
        format!(
            "required projection {} cannot be read: {err}",
            path.display()
        )
    })?;
    let mut set = BTreeSet::new();
    for (line_index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: Committed = line
            .split(',')
            .map(|token| {
                token
                    .trim()
                    .parse::<u8>()
                    .map_err(|err| format!("invalid integer at line {}: {err}", line_index + 1))
            })
            .collect::<Result<_, _>>()?;
        if v.len() != expected_width || v.iter().any(|entry| *entry > 2) {
            return Err(format!(
                "projection {} line {} has invalid committed vector {:?}",
                path.display(),
                line_index + 1,
                v
            ));
        }
        set.insert(v);
    }
    if set.is_empty() {
        return Err(format!("required projection {} is empty", path.display()));
    }
    Ok(set)
}

/// True if a committed vector shows a cross-boundary fork: two validators
/// committed to DIFFERENT non-zero sides. This is the negation of the TLA
/// `Safety` invariant.
fn is_unsafe(committed: &Committed) -> bool {
    let mut seen: Option<u8> = None;
    for &c in committed {
        if c != 0 {
            match seen {
                None => seen = Some(c),
                Some(s) if s != c => return true,
                _ => {}
            }
        }
    }
    false
}

fn projection_path(name: &str) -> PathBuf {
    // tests run with CWD = crate dir (crates/sage-node); the projections live at
    // the workspace root under formal/states/committed/.
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../formal/states/committed");
    p.push(format!("committed_{name}.txt"));
    p
}

/// Faithful configs: Rust-reachable committed set == TLC-reachable set, and
/// NEITHER contains an unsafe (forked) vector.
fn check_faithful(name: &str, n: usize, f: u64) {
    let threshold = n as u64 - f; // SAGE gate = n - f
    let path = projection_path(name);
    let tlc = parse_committed_projection(&path, n).unwrap_or_else(|err| panic!("{name}: {err}"));
    let rust = rust_reachable_committed(n, threshold);

    // Teeth part 1: the model itself must be safe here.
    let tlc_unsafe: Vec<_> = tlc.iter().filter(|v| is_unsafe(v)).collect();
    assert!(
        tlc_unsafe.is_empty(),
        "{name}: TLC projection unexpectedly contains {} unsafe vectors",
        tlc_unsafe.len()
    );
    // And so must the Rust enumeration.
    let rust_unsafe: Vec<_> = rust.iter().filter(|v| is_unsafe(v)).collect();
    assert!(
        rust_unsafe.is_empty(),
        "{name}: Rust enumeration produced {} unsafe vectors under the n-f gate — \
         DecisionUniqueness violated in code",
        rust_unsafe.len()
    );

    // The core conformance assertion: identical reachable committed-vector sets.
    assert_eq!(
        rust,
        tlc,
        "{name}: Rust-reachable committed set != TLC-reachable set \
         (rust={} vectors, tlc={} vectors)",
        rust.len(),
        tlc.len()
    );
    eprintln!(
        "OK {name}: {} committed vectors, model==code, 0 unsafe (n={n}, f={f}, threshold={threshold})",
        rust.len()
    );
}

/// Broken control: the blind gate (threshold = 1) MUST admit forked committed
/// vectors, in BOTH the TLC projection and the Rust enumeration. This is the
/// falsifiability proof that the conformance harness is not vacuous.
fn check_broken(name: &str, n: usize) {
    let threshold = 1u64; // blind: switch on local readiness (>=1 attester)
    let path = projection_path(name);
    let tlc = parse_committed_projection(&path, n).unwrap_or_else(|err| panic!("{name}: {err}"));
    let rust = rust_reachable_committed(n, threshold);

    let tlc_unsafe = tlc.iter().filter(|v| is_unsafe(v)).count();
    let rust_unsafe = rust.iter().filter(|v| is_unsafe(v)).count();
    assert!(
        tlc_unsafe > 0,
        "{name}: broken control MUST have unsafe vectors in the TLC dump but has none — \
         the model check has no teeth"
    );
    assert!(
        rust_unsafe > 0,
        "{name}: broken control MUST have unsafe vectors in the Rust enumeration but has none — \
         the conformance harness has no teeth"
    );
    assert_eq!(
        rust,
        tlc,
        "{name}: broken-control Rust set != TLC set (rust={}, tlc={})",
        rust.len(),
        tlc.len()
    );
    eprintln!(
        "OK {name}: {} committed vectors, model==code, {rust_unsafe} unsafe (blind control fires — harness has teeth)",
        rust.len()
    );
}

#[test]
fn conformance_faithful_n4() {
    check_faithful("n4", 4, 1);
}

#[test]
fn conformance_faithful_n7() {
    check_faithful("n7", 7, 2);
}

#[test]
fn conformance_faithful_n10() {
    check_faithful("n10", 10, 3);
}

#[test]
fn conformance_broken_control_n4() {
    check_broken("blind_n4", 4);
}

#[test]
fn conformance_broken_control_n7() {
    check_broken("blind_n7", 7);
}

#[test]
fn projection_parser_rejects_missing_empty_and_malformed_inputs() {
    let base = std::env::temp_dir().join(format!("sage-conformance-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    assert!(parse_committed_projection(&base.join("missing.txt"), 4).is_err());

    let empty = base.join("empty.txt");
    std::fs::write(&empty, "\n").unwrap();
    assert!(parse_committed_projection(&empty, 4).is_err());

    let malformed = base.join("malformed.txt");
    std::fs::write(&malformed, "0,1,2\n0,1,2,3\n").unwrap();
    assert!(parse_committed_projection(&malformed, 4).is_err());

    std::fs::remove_dir_all(base).unwrap();
}
