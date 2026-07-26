# SAGE Bounded Model Check (TLA+ / TLC)

This directory contains three bounded model-check artifacts used by paper
Appendix `app:spec`. `Sage.tla` exhaustively checks CutCert-side boundary
agreement and decision uniqueness for n ∈ {4,7,10} under an assumed legacy
boundary fence. `ManifestAgreement.tla` checks complete-manifest agreement for
one epoch under an n−f `ManifestCertificate`, with a falsifying legacy
single-producer control. `SageRollback.tla` separately checks no absolute
reversion and bounded provisional state. The models do not verify
committee-wide fence installation, certificate dissemination or manifest
gossip, or AbortCert/SealCert coordination.

## Files

| File | Purpose |
|---|---|
| `Sage.tla` | Controller LTS: dual-run → quorum-gated cutover under a binary partition. One `QuorumGated` constant toggles faithful (TRUE) vs blind control (FALSE). |
| `Sage_n4.cfg`, `Sage_n7.cfg`, `Sage_n10.cfg` | Faithful configs (`QuorumGated=TRUE`). Must HOLD. |
| `SageBlind_n4.cfg`, `SageBlind_n7.cfg` | Blind control (`QuorumGated=FALSE`, no quorum gate). Must FAIL with a Safety counterexample. |
| `ManifestAgreement.tla` | Complete-manifest acceptance for one epoch: shares bind the complete payload, correct validators share at most one body, adoption requires n−f shares, and the local epoch lock retains the first adopted body. |
| `ManifestAgreement.cfg` | Faithful multi-share manifest config (`SingleProducerAuth=FALSE`, n=4, f=1). Must HOLD `CompleteManifestAgreement`. |
| `ManifestAgreementBroken.cfg` | Legacy single-producer control (`SingleProducerAuth=TRUE`). Must FAIL with a `CompleteManifestAgreement` counterexample in which two bodies reuse one certified boundary but differ in tool hash and replay-context root. |
| `SageRollback.tla` | Two-tier-finality rollback LTS: abort (guard `g_2`) discards only the provisional suffix; `AllowAbsoluteReversion` toggles faithful (FALSE) vs broken control (TRUE). |
| `SageRollback.cfg` | Faithful rollback config. Must HOLD (`NoAbsoluteReversion` + `ProvisionalBounded`). |
| `SageRollbackBroken.cfg` | Broken control (reverts an absolute block). Must FAIL with a `NoAbsoluteReversion` counterexample. |
| `SageDistinctCommittee.tla` | Relaxes the identical-committee restriction (MC04): a migration committee `V_M` and a target committee `V_T` that may partially overlap or be disjoint. Readiness shares are signed by `V_M` members; `V_T` members install the boundary. `JointGated` toggles the conservative joint threshold (TRUE) vs sizing the gate against `V_T` alone (FALSE). |
| `SageDistinctCommittee_partial.cfg` | Partial overlap (NM=4, NT=4, Overlap=2). Must HOLD. |
| `SageDistinctCommittee_disjoint.cfg` | Fully disjoint committees (Overlap=0). Must HOLD. |
| `SageDistinctCommitteeBroken.cfg` | Broken control: gate sized against the smaller target committee alone (NM=7, NT=4, FT=1 ⇒ threshold 3 ≤ ⌊7/2⌋). Must FAIL — two disjoint sides each reach 3 attesters inside `V_M`. |
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
| `Safety` (`Sage.tla`) | CutCert-side boundary agreement under the assumed legacy fence | No two validators choose different target boundary blocks. Fence installation itself is not modeled. |
| `DecisionUniqueness` (`Sage.tla`) | Thm. `th:unique` (Decision uniqueness) + Lem. `lem:qi` (Quorum intersection) | At most one side can form an (n−f) CutCert, because two disjoint (n−f) sets cannot both fit in n validators when n ≥ 3f+1. |
| `CompleteManifestAgreement` (`ManifestAgreement.tla`) | Thm. `th:unforge` (complete-manifest integrity and agreement) | At most one distinct complete-manifest body is adopted by correct validators in one epoch when n−f shares bind the complete payload and correct validators share at most one body. |
| `NoAbsoluteReversion` (`SageRollback.tla`) | Thm. `th:rollback` (Rollback correctness) + `check_no_absolute_reversion` | An abort discards only the provisional suffix; no absolutely-final (legacy or sealed) block is ever reverted. |
| `ProvisionalBounded` (`SageRollback.tla`) | two-tier finality model | Provisional blocks live only in [h_c, h_r) until the seal. |
| `Safety`, `DecisionUniqueness` (`SageDistinctCommittee.tla`) | MC04 residual: theorems stated for `V_M = V_1 = V_2` | Boundary agreement and cutover-certificate uniqueness still hold when the migration and target committees are non-identical, provided the gate uses the conservative joint threshold `max(|V_M|-f_M, |V_T|-f_T)`. This is a **bounded model result only**: the Rust runtime still assumes one identical committee, so it does not license a distinct-committee deployment claim. |

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

2. **Falsifiability via broken controls.** `Sage.tla` with
   `QuorumGated=FALSE` removes the quorum gate (a validator switches on its own
   local readiness — the dead-quorum-gate behaviour the empirical M3 run exposed,
   where SAGE forked 5/5 before the gate was wired in). TLC **must** report a
   `Safety` counterexample for the blind control. If the blind control passed,
   the check would be vacuous. `run_tlc.sh` asserts the blind control FAILS, so a
   regression that accidentally made Safety trivially true would break the gate.
   Independently, `ManifestAgreement.tla` with `SingleProducerAuth=TRUE`
   replaces the n−f complete-payload share threshold with the pre-MC02 single
   producer signature. TLC must then exhibit two correct validators adopting
   different bodies with the same certified boundary. This distinguishes
   complete-manifest agreement from CutCert-only boundary agreement.

## Scope and threats to validity

- **Bounded**, not a general proof: n ∈ {4,7,10}, f = ⌊(n−1)/3⌋, a binary
  partition, and the cutover decision. This matches the paper's stated bounded
  scope; a full inductive proof over all n remains future work (stated as such).
- The model abstracts the *decision* layer (attestation → CutCert → switch). The
  per-engine consensus safety of PoA and HotStuff is assumed (paper
  Assumption `as:engines`), as in the proofs — the model checks the *boundary*,
  which is SAGE's contribution.
- `ManifestAgreement.tla` is a bounded component theorem for one n=4, f=1
  equal-power epoch. It models local receipt, complete-payload shares, manifest
  authentication, and the durable one-body epoch lock. It does not model
  reliable dissemination, network gossip, or live multi-host convergence.
- Rollback safety (Thm. `th:rollback`) is checked by `SageRollback.tla` with an
  explicit abort action (guard `g_2`) over the two-tier finality model, plus a
  broken control (`AllowAbsoluteReversion=TRUE`) that TLC must catch. It is a
  separate module rather than folded into `Sage.tla` to keep each module's state
  space small and its property focused.
