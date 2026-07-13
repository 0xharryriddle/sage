# Debate Outcome — SAGE "finish the project" plan audit

**Date:** 2026-07-12
**Trigger:** Model switch cx/gpt-5.6-sol → claude-opus-4.8-thinking; user asked to audit + debate the
plan gpt-5.6-sol had been executing, using an advisor/executor (debate) pattern.

## Execution mode
- Advisor: `cx/gpt-5.6-sol` via `9Router-Proxy` (localhost:20128), real `hermes chat -Q` subprocess,
  run in-repo so it verifies claims against real code. Independent first pass, sealed packet.
- Synthesizer/executor: claude-opus-4.8-thinking (this session).
- **Honesty limit:** `hermes chat -Q` has no `--reasoning-effort` flag, so "max" effort could NOT be
  programmatically pinned on the subprocess reviewer. It ran at the proxy default for that Codex
  variant. Not claimed otherwise. Heterogeneity (different model family + independent code reads) is real.
- Shared-bias risk: LOW (different model families, both cited exact file:line against the same live repo).

## Verified grounding findings (all confirmed against real code + paper by BOTH models)

| ID | Finding | Evidence | Status |
|----|---------|----------|--------|
| A | The VPS testbed binary uses NO store backend at all. `FileBackend` is dead code relative to what runs on the VPS. | `validator_proc.rs:148` `ProcessValidator::new(id,cfg,transport)`, zero backend refs; `lib.rs:160` `restore_from_store(&mut self, store: MemoryBackend)` hardcoded to MemoryBackend | Verified |
| B | The paper EXPLICITLY scopes out durable disk recovery as future work. | `paper.tex:116` "restart/continue recovery from in-memory snapshots"; `:2648` "Durable disk-backed crash/restart recovery remains future work"; `:2755` lists it under Future Work | Verified |
| C | `FileBackend` is atomic full-snapshot replacement (whole StoreImage → JSON every commit), NOT the "WAL-backed" store the session summary claimed. Commit cost grows with total stored state. | `file.rs:75-91` serialize whole image + tmp + fsync + rename + parent fsync; `file.rs:97` clones entire image per mutation | Verified |
| D | The CutCert crash window is real but is NOT a submission blocker (see B). The atomic primitive to close it already exists. | `lib.rs:965` `put_migration_decision` written separately AFTER boundary finalization; `file.rs:234` `commit_transition` can already fold the decision into one atomic image | Verified (partial: real bug, wrong priority) |
| E | Ed25519 prose imprecision at `paper.tex:2656`: says "the simulator realizes the cutover certificate as an n−f Ed25519 multi-signature". The real-host TESTBED uses REAL Ed25519 (`deploy_multihost.sh:56`, `netns_testbed.sh:37` build `--features real-crypto`); the deterministic experiment SIMULATOR uses a modeled scheme (`authenticates_signer()==false`). Prose conflates the two. | grep of scripts + paper | Verified — prose fix, not a rerun blocker |

## Core verdict: the plan's dependency ordering was INVERTED
The plan asserted "VPS rerun and paper update are strictly blocked until transactional disk storage is
implemented." This is **false** for the current artifact and paper scope (Findings A + B). gpt-5.6-sol,
shown the evidence, **retracted its own critical path** and agreed the disk backend is post-submission
hardening, not the gate.

The disk-storage work of the last several sessions (FileBackend, CommittedTransition, transact) is
correct engineering but is **scope creep against a paper that already disclaims durable disk recovery**.
It is not wired into the binary that runs on the VPS and blocks nothing on the submission path.

## Over-engineering both models now agree to CUT from the active path
- Making durable disk storage the universal P0 blocker.
- Labeling the full-image atomic store "WAL-backed".
- Building a scalable WAL before any accepted claim needs durable recovery.
- Treating the CutCert persistence window as submission-critical absent a durable-restart claim.
- Production-grade HotStuff hardening beyond the safety/behavior actually claimed.
- Wiring persistent storage into TPS campaigns (full-image commits could DISTORT the benchmark).

## Corrected critical path (accepted)
1. **Claim-to-evidence matrix** — every abstract/eval/limitations/conclusion claim → reproducible
   command + retained artifact. Explicitly classify disk recovery + production HotStuff as out-of-scope.
   *Highest-value next action.*
2. **Resolve Ed25519 claim (E)** — narrow paper.tex:2656 prose (testbed=real Ed25519, sim=modeled), and
   confirm every signature-dependent cited campaign ran with `real-crypto`.
3. **Rerun byte-identical sim baselines + competitor campaigns** — retain configs/seeds/commit/logs/
   status. Align paired-seed statistics + trial count (30 vs paper's 60).
4. **Rerun VPS fork-differential + TPS** via the EXISTING `validator_proc` path (no FileBackend).
5. **Reconcile every paper number + competitor label** against retained outputs; regenerate tables/
   figures with input hashes; explain every delta.
6. **Submission gate** — fmt, workspace tests w/ paper feature set, no-default-feature, ignored
   authoritative testbed tests, verifier 18/18, formal conformance, experiment verifier, negative
   controls, git diff --check. Archive commit + toolchain.
7. **Paper update** — reconciled evidence, rebuild PDF, consistency checks, reproducibility manifest.
8. **DEFERRED (post-submission):** close CutCert window (fold decision into commit_transition —
   primitive exists), generalize node store type, wire FileBackend into validator_proc, crash-injection
   tests, consider WAL/delta store.

## Dissent preserved
None unresolved. gpt-5.6-sol's only residual conditions ("what would change my ordering"): evidence that
the submission/tests/paper elsewhere claim durable process crash/restart, persistent CutCert authority,
WAL behavior, or VPS recovery from disk; or that `validator_proc` is not the real VPS binary. All three
checked and refuted (Findings A, B). Ordering stands.

## Human escalation
One decision belongs to the user (see chat): confirm the submission scope treats durable disk recovery +
production HotStuff as explicitly out-of-scope (as the paper already states), so storage work stays
deferred. If a target venue's artifact-evaluation requires durable crash recovery, storage returns to the
critical path.
