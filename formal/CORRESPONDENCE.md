# TLA+ ↔ Rust Conformance (machine-checked)

This document records the **machine-checked** correspondence between the bounded
TLA+ model (`formal/Sage.tla`) and the SAGE Rust runtime. It backs paper
Appendix `app:spec` and answers Reviewer 2's rebuttal question Q8 (TLA↔code
linkage).

Before this artifact the paper claimed only a *hand*-correspondence between the
model and the code. It is now upgraded to a **bounded-trace conformance check**:
the same decision logic the runtime uses is replayed over the *identical* bounded
scenario space TLC explores, and the two are asserted to reach the same set of
committed outcomes — with a broken control that must diverge in **both** the
model and the code, so the check cannot pass vacuously.

## Scope and honest boundary

This is **bounded conformance, NOT a mechanized refinement proof.** We verify
that the Rust cutover-gate decision logic and the TLA+ model agree on the set of
reachable `committed` outcomes over `n ∈ {4,7,10}` and **all** partition splits.
We do NOT prove the full Rust implementation refines the spec for all executions
(that is a separate, much larger undertaking, listed as future work). The claim
we make — and the only one — is the bounded committed-set equivalence below.

## What is checked

| TLA+ (`formal/Sage.tla`) | Rust | Test |
|---|---|---|
| `CutoverEnabled` quorum arm `SideHasQuorum(s) == Cardinality(SideAttesters(s)) >= Quorum` | `cutover_gate::gate_open` (the exact predicate `drive_quorum_cutover` calls in `crates/sage-node/src/lib.rs`) | `sage-node/tests/formal_conformance.rs` |
| `SideAttesters(s)` same-side filter under partition | `cutover_gate::side_has_cutcert` | `formal_conformance.rs` |
| `DecisionUniqueness == ~(SideHasQuorum(1) /\ SideHasQuorum(2))` | `cutover_gate::two_sides_can_switch` (its negation) | `formal_conformance.rs` |
| `Safety` invariant (no two distinct committed boundary blocks) | committed-vector "no `1` and `2` coexist" check | `formal_conformance.rs` |
| Full reachable `committed` state set | Rust reachable-`committed` enumerator | `formal_conformance.rs` (set equality) |
| ACTIVATING authority transfer (abstracted out of `Sage.tla` by design) | `MigrationPhase` machine `can_transition_to` / `legacy_is_authoritative` | `sage-controller/src/phase.rs::authority_transfer_is_unique_and_gated_on_activation` |

## The decisive numbers (from `make conformance`)

TLC dumps every reachable state; we project each to its `committed` vector and
take the distinct set. The Rust enumerator, driving the **real** `cutover_gate`
predicate, reproduces that set exactly:

| Config | Gate | Distinct committed vectors (model == code) | Unsafe (fork) vectors |
|---|---|---|---|
| `n4`        | faithful `n-f=3` | 31   | **0** |
| `n7`        | faithful `n-f=5` | 255  | **0** |
| `n10`       | faithful `n-f=7` | 2047 | **0** |
| `blind_n4`  | broken (local)   | 81   | 50 |
| `blind_n7`  | broken (local)   | 2187 | 1932 |

- **Faithful gate:** 0 unsafe committed vectors at every `n` — the `n-f` quorum
  gate makes a cross-boundary fork *unreachable* in both the model and the code.
- **Broken control:** ≥1 unsafe vector — the harness has teeth. A gate that
  switched on local readiness (threshold 1) admits the fork, and the Rust
  enumerator and TLC agree it does. This mirrors the empirical M3 testbed result
  (blind hard fork forks 5/5, SAGE 0/5).

## Reproduce

```bash
make conformance
# = bash formal/dump_states.sh                      # TLC -> committed-vector projections
#   cargo test -p sage-node --test formal_conformance  # model == code, all 5 configs
#   cargo test -p sage-controller phase::tests::authority_transfer  # ACTIVATING invariant
```

The raw TLC state dumps (up to ~800 MB at n=10) are transient and git-ignored;
only the compact committed-vector projections in `formal/states/committed/`
(≈92 KB total) are kept as the checked-in artifact. `dump_states.sh` regenerates
them deterministically from `Sage.tla`.
