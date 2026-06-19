# SAGE Design Implications from Research

This document distills the research into concrete design requirements for SAGE's Rust implementation.

## 1. System Architecture Decisions

### Decision 1: Rust Workspace with Pure Protocol Core

SAGE should be a Cargo workspace with protocol crates that are deterministic and independent from networking, storage, and runtime choices.

Required separation:

- `sage-core`: types, state, blocks, roots, errors.
- `sage-consensus`: engine traits, PoA, HotStuff-like target, quorum rules.
- `sage-controller`: migration state machine and invariants.
- `sage-manifest`: manifest and certificate verification.
- `sage-sim`: deterministic event-driven simulator.
- `sage-stats`: statistical utilities.
- `sage-experiments`: experiment binaries.
- Later: `sage-network`, `sage-store`, `sage-node`.

### Decision 2: Epoch and Config ID Everywhere

Every safety-relevant object must include `chain_id`, `epoch`, and/or `config_id`:

- blocks;
- votes;
- QCs;
- readiness attestations;
- abort attestations;
- manifests;
- checkpoints;
- client requests;
- state-transfer proofs.

This prevents replay across migrations and prevents stale validators from participating in new epochs.

### Decision 3: Quorum Policy as a Type

Quorums must not be hardcoded as constants scattered through code. SAGE needs explicit policies:

- majority quorum for PoA-like engines;
- BFT quorum for HotStuff-like engines;
- `n-f` migration cutover quorum;
- `n-f` abort quorum;
- future weighted quorums.

Startup validation must verify intersection assumptions for the configured policy.

### Decision 4: Manifest as the Boundary Object

The migration manifest is the canonical boundary artifact. It must bind:

- chain id;
- migration epoch;
- cutover height;
- parent block hash;
- boundary state root;
- source engine generation;
- target engine generation;
- validator set/config id;
- tool/build hash;
- CutCert;
- manifest signature or aggregate proof.

### Decision 5: Deterministic Simulation Before Real Networking

The first working system should be an executable protocol artifact with adversarial simulation. Real networking should come after safety invariants are well-tested.

## 2. Protocol Requirements

### 2.1 Dual-Run

- Legacy engine is the only finalizer during dual-run.
- Target engine runs in shadow mode and cannot propose or finalize.
- Shadow validation must be side-effect-free against the canonical state.
- Shadow verdicts are recorded with height, root, validator id, epoch, and engine generation.

### 2.2 Readiness and Cutover

- Readiness requires `kappa` consecutive valid shadow verdicts with matching roots.
- Correct validators sign at most one readiness attestation per epoch.
- CutCert requires `n-f` attestations over the same tuple.
- Cutover initializes the target engine from the boundary root and bootstrap certificate.
- If partition prevents a CutCert, dual-run extends rather than splitting.

### 2.3 Rollback

- Blocks in `[h_c, h_r)` are provisionally final.
- Blocks before `h_c` and after sealing are absolutely final.
- Abort before `h_r` discards only the provisional suffix.
- Abort after `h_r` is refused.
- Rollback resumes from the retained legacy certificate at `h_c - 1`.
- Suffix transactions are deterministically replayed or returned to mempool according to explicit policy.

### 2.4 Failure and Adversary Model

The simulator must model:

- network partitions straddling cutover;
- delayed messages before GST;
- crash/recover validators;
- Byzantine readiness attempts;
- latent state divergence;
- target-engine stall;
- manifest mismatch;
- stale epoch messages;
- duplicate signatures and equivocation.

## 3. Implementation Priorities

### Priority 1: Safety-Critical Data Types

Implement and test:

- block/header hash;
- state root;
- quorum policy;
- certificate;
- manifest;
- migration schedule;
- finality status;
- controller phase.

### Priority 2: Controller State Machine

Build SAGE as a guarded transition system before building full consensus engines. This gives early tests for:

- phase precedence;
- abort dominance;
- readiness streaks;
- cutover uniqueness;
- rollback deadline;
- sealing.

### Priority 3: Minimal Engines

Implement minimal PoA and HotStuff-like engines sufficient to generate and validate finalized blocks. Keep the target engine modular so it can be made more realistic later.

### Priority 4: Adversarial Simulator

Build a deterministic event queue and conflict detector. The simulator must be able to produce failures for broken protocols; otherwise it is not faithful enough.

### Priority 5: Reproducibility Harness

Every experiment writes raw records, generated tables, and invariant checks. Paper numbers must come from generated output.

## 4. Engineering Constraints

- All consensus-critical serialization must be canonical.
- All hash/signature payloads must be domain-separated.
- Safety-critical state must be persisted before external messages in real-node mode.
- Simulated cryptography must be isolated behind the same traits used by real cryptography.
- No module should silently infer epoch or engine generation from global state; pass it explicitly.

## 5. Revised Build Order

1. Create workspace and crates.
2. Implement canonical encoding and hash domains.
3. Implement core types and state roots.
4. Implement quorum policies and certificate validation.
5. Implement manifest construction and verification.
6. Implement controller state machine with property tests.
7. Implement minimal PoA finalizer.
8. Implement minimal HotStuff-like target and shadow validation.
9. Implement deterministic event-driven simulator.
10. Implement baselines and adversarial scenarios.
11. Implement RQ experiments and stats.
12. Add `make verify`.
13. Add storage and networking traits.
14. Add local multi-validator runtime.
15. Add real crypto backend and persistence hardening.

## 6. Success Criteria

The implementation is credible when:

- SAGE has zero conflicting commits in configured adversarial partition trials.
- Hard fork and blind cutover produce conflicts under the same model.
- Stop-the-world has measurable downtime.
- Reconfiguration-only cannot complete a protocol swap.
- In-window rollback succeeds and past-deadline rollback fails.
- Manifest replay and wrong-root manifests are rejected.
- All numbers in the paper can be regenerated from result files.
- The simulator can run many seeds quickly and deterministically.
