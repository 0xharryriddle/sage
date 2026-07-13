# Cover Letter — SAGE Submission

To the Editor and Reviewers,

We submit *SAGE: Shadow-Anchored Graceful Evolution*, a protocol for migrating a
live permissioned blockchain between two heterogeneous consensus engines
(e.g. PoA → HotStuff) without halting block production, with machine-checked
safety and a real multi-host evaluation.

## What SAGE contributes

The specific contribution is the **`n−f` cutover-quorum gate for the
*heterogeneous* engine boundary** — the point at which the fault model and quorum
family change. We prove and empirically isolate that this threshold, not merely
"having a gate," is what prevents a cross-boundary fork.

## Summary of evidence tiers (honestly scoped)

- **Proofs**: cross-boundary safety, decision uniqueness, quantified zero-halt
  liveness (with an explicit ACTIVATING authority-transfer state), rollback
  safety (unconditional) vs automatic-replay liveness (conditional), and manifest
  unforgeability/replay as cryptographic reductions.
- **Bounded model check (TLA+/TLC)**: safety + uniqueness + no-absolute-reversion
  + replay-context fail-closed over n ∈ {4,7,10}, each paired with a broken
  control TLC must (and does) falsify.
- **Multi-process testbed**: real OS processes, real sockets. Under a 3/3
  partition at cutover the blind hard fork forks 10/10 and SAGE forks 0/10.
- **Real multi-host (6 independent cloud VMs, real TCP)**: safety differential
  SAGE 0/5 vs hard fork 5/5, holding under injected ~104 ms inter-VM RTT
  (0/3 vs 3/3); **committed-throughput** parity (SAGE 777 vs hard fork/Cox-style
  790 TPS, a 1.65% tie) with a cutover handoff gap ~10.5 ms, an order of
  magnitude below the steady-state block cadence; scaled to n=12 (2 validators
  per host) with zero fork.

## On the closest SOTA (Cox, BCRA 2026)

We take the SOTA comparison seriously rather than dismissing it. Cox is a
homogeneous BFT↔BFT live switch; its own paper scopes it to a uniform quorum
family (we quote this verbatim). We implement **faithful Cox** — gating on Cox's
*actual* 2f+1 checkpoint quorum — and show that even Cox's real gate forks 5/5
at the heterogeneous boundary (because 2(2f+1) ≤ n once n > 3f+1), while SAGE's
n−f gate forks 0/5. This isolates the contribution against a non-strawman baseline.

## What we explicitly do NOT claim

- No cross-region *geo-distributed* throughput: the multi-host runs are
  same-region (WAN latency injected via `tc netem`). This is stated as future work.
- The BLS constant-size cert column is an analytic target; the Ed25519 O(n) column
  is measured.
- Assumption-3 admissibility (A≈89%) is a modeled estimate from a published EVM
  transaction distribution, not a live-trace measurement; rollback safety is
  fail-closed for any A regardless.
- The formal guarantees come from a bounded TLA+ abstraction + Rust tests, not a
  mechanized proof of the running Rust (a model-vs-code gap we disclose).

## Reproducibility

Every claim maps to a command in `docs/ARTIFACT_EVALUATION.md` and `REPRODUCE.md`.
The core gate (`cargo fmt`/`clippy`/`test`/`verify` + `make formal`) runs in
minutes on a laptop with no special hardware. Harnesses that need real hosts
refuse to fabricate data without a real `config/hosts.txt`.

We thank the reviewers for their time and the detailed feedback that shaped this
revision; a point-by-point mapping is in `docs/RESPONSE_TO_REVIEWS.md`.

Sincerely,
The Authors
