# SAGE Bounded Model Check (TLA+ / TLC)

This directory holds the bounded model-check artifact that backs the paper's
claim (App. `app:spec`):

> "A bounded-model check over n ∈ {4,7,10} and all partition splits of the
> cutover window reproduces I and the rollback property."

It closes **Path A item 2** in `docs/reviews/PUBLISHABILITY_ASSESSMENT.md` (the
"machine-checked (bounded)" wording, blocker P0-1) and complements the
multi-process observed-fork testbed (`scripts/m3_partition_experiment.sh`,
Path A item 1). The empirical testbed shows a *real* fork; this model check
shows the safety property holds *exhaustively* over the bounded state space.

## Files

| File | Purpose |
|---|---|
| `Sage.tla` | Controller LTS: dual-run → quorum-gated cutover under a binary partition. One `QuorumGated` constant toggles faithful (TRUE) vs blind control (FALSE). |
| `Sage_n4.cfg`, `Sage_n7.cfg`, `Sage_n10.cfg` | Faithful configs (`QuorumGated=TRUE`). Must HOLD. |
| `SageBlind_n4.cfg`, `SageBlind_n7.cfg` | Blind control (`QuorumGated=FALSE`, no quorum gate). Must FAIL with a Safety counterexample. |
| `SageRollback.tla` | Two-tier-finality rollback LTS: abort (guard `g_2`) discards only the provisional suffix; `AllowAbsoluteReversion` toggles faithful (FALSE) vs broken control (TRUE). |
| `SageRollback.cfg` | Faithful rollback config. Must HOLD (`NoAbsoluteReversion` + `ProvisionalBounded`). |
| `SageRollbackBroken.cfg` | Broken control (reverts an absolute block). Must FAIL with a `NoAbsoluteReversion` counterexample. |
| `run_tlc.sh` | Runs all configs and asserts the expected verdicts. CI gate. |
| `tla2tools.jar` | TLC 2.19 (vendored so the check is self-contained). |

## Run

```bash
bash formal/run_tlc.sh
```

Requires `java` (tested on OpenJDK 21). Each faithful config explores all
partition splits; n=10 is ~3.4M distinct states (~27s on 4 workers).

## Property → paper-theorem mapping

| TLA+ property (`Sage.tla`) | Paper | Meaning |
|---|---|---|
| `Safety` (`Sage.tla`) | Thm. `th:safety` (Cross-boundary safety), invariant `I` | No two validators commit different boundary blocks. |
| `DecisionUniqueness` (`Sage.tla`) | Thm. `th:unique` (Decision uniqueness) + Lem. `lem:qi` (Quorum intersection) | At most one side can form an (n−f) CutCert, because two disjoint (n−f) sets cannot both fit in n validators when n ≥ 3f+1. |
| `NoAbsoluteReversion` (`SageRollback.tla`) | Thm. `th:rollback` (Rollback correctness) + `check_no_absolute_reversion` | An abort discards only the provisional suffix; no absolutely-final (legacy or sealed) block is ever reverted. |
| `ProvisionalBounded` (`SageRollback.tla`) | two-tier finality model | Provisional blocks live only in [h_c, h_r) until the seal. |

## Faithfulness (why this is not a vacuous pass)

A formal model is worthless if it bakes in the property it claims to check. Two
guards against that:

1. **The model mirrors the implementation.** Cutover requires an (n−f) CutCert
   formed from *same-side* readiness attestations — exactly what the runtime
   `drive_quorum_cutover` counts (`crates/sage-node/src/lib.rs`) and what the
   paper's CutCert definition requires. The partition drops cross-side messages
   (a side only sees its own attestations), mirroring the socket-layer partition
   the testbed engages via `Transport::partition_to`. Two sides committing
   different boundary blocks is the exact observed fork the blind testbed control
   produced.

2. **Falsifiability via a blind control.** The *same* module with
   `QuorumGated=FALSE` removes the quorum gate (a validator switches on its own
   local readiness — the dead-quorum-gate behaviour the empirical M3 run exposed,
   where SAGE forked 5/5 before the gate was wired in). TLC **must** report a
   `Safety` counterexample for the blind control. If the blind control passed,
   the check would be vacuous. `run_tlc.sh` asserts the blind control FAILS, so a
   regression that accidentally made Safety trivially true would break the gate.

## Scope and threats to validity

- **Bounded**, not a general proof: n ∈ {4,7,10}, f = ⌊(n−1)/3⌋, a binary
  partition, and the cutover decision. This matches the paper's stated bounded
  scope; a full inductive proof over all n remains future work (stated as such).
- The model abstracts the *decision* layer (attestation → CutCert → switch). The
  per-engine consensus safety of PoA and HotStuff is assumed (paper
  Assumption `as:engines`), as in the proofs — the model checks the *boundary*,
  which is SAGE's contribution.
- Rollback safety (Thm. `th:rollback`) is checked by `SageRollback.tla` with an
  explicit abort action (guard `g_2`) over the two-tier finality model, plus a
  broken control (`AllowAbsoluteReversion=TRUE`) that TLC must catch. It is a
  separate module rather than folded into `Sage.tla` to keep each module's state
  space small and its property focused.
