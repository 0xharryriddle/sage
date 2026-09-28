# SAGE — Shadow-Anchored Graceful Evolution

SAGE migrates a live permissioned blockchain from one consensus engine to another
without halting block production. This repository is a Rust protocol artifact
covering deterministic simulation, experiment generation, manifest validation,
and an early local multi-validator runtime.

## Current manuscript

**SAGE: A Certified Cross-Engine Boundary for Fixed-Committee Consensus
Migration in Permissioned Blockchains** is the current manuscript at
`docs/paper/SAGE_ThaiCong_IEEE/main.tex`. It uses Elsevier `elsarticle` for
*Blockchain: Research and Applications*; `_IEEE` in the path is historical.
The earlier `docs/paper.tex` remains a separate legacy draft. The manuscript
presents a conditional locked-readiness design, not a fully implemented
networked handoff or a submission-ready finding. Its presentation build pins
eight CSV inputs, with three conflicting legacy inputs retained unchanged and
the paper-specific versions under `results/raw/paper/SAGE_ThaiCong_IEEE/`.
`rebuild.py --check-data` checks pins; `--verify-rebuild` compares two builds
and 22 products. Neither reruns historical campaigns or grants runtime authority.

The additional `formal/` design models are bounded abstractions, not
implementation or deployment authority. See `formal/README.md` for their
faithful and falsifying-control configurations. Candidate Retry runtime
implementation remains outside this branch pending independent exact-byte
authorization.

## Quick Start

```bash
cargo build --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo run -p sage-experiments --bin verify -- --config config/default.toml
cargo run -p sage-experiments --bin run_rq1 -- --config config/rq1.toml --seeds 0,1,2
```

## What SAGE Does

Permissioned blockchains need to swap consensus engines (for example, PoA to
HotStuff) as trust models and performance requirements change. SAGE performs
this migration without halting block production, using four mechanisms:

1. **Single-finalizer dual-run**: the legacy engine remains the sole finalizer
   while the target engine validates in shadow mode.
2. **Shadow anchoring**: validators collect `kappa` consecutive matching shadow
   verdicts before certifying readiness.
3. **Quorum-certified cutover**: `n - f` validators certify readiness before
   authority transfers to the target engine.
4. **Bounded rollback**: if cutover fails, the system can roll back to the last
   legacy-committed block within a deadline window.

## Comparing Strategies

The artifact compares six migration strategies:

| Strategy | Protocol swap | Downtime | Fork risk under partition |
|----------|---------------|----------|---------------------------|
| SAGE (proposed) | yes | 0 | 0 observed (0/10 in multi-process testbed) |
| SageBlind | yes | 0 | baseline-risk strategy |
| Stop-the-world | yes | expected finality gap | 0 |
| Hard fork (flag-day) | yes | 0 | forks 10/10 in multi-process testbed |
| Reconfig-only | no | 0 | 0 |
| Cox-style (live-switch SOTA) | yes | 0 (ties SAGE, p=1.0) | forks 10/10 in multi-process testbed |

Cox-style models the closest live-switch SOTA class (Cox, Blockchain: Research and Applications
2026) on our engine interface: a zero-downtime switch on a quorum-certified boundary checkpoint,
without SAGE's n-f dual-run cutover gate. It ties SAGE on migration cost and separates only on
adversarial safety, isolating the n-f gate. See `scripts/m6_sota_baseline_differential.sh`.

## Workspace Structure

```text
sage/
  Cargo.toml                     # workspace root
  Makefile                       # build/test/experiment targets
  config/                        # default, rq1-rq4, safety TOML configs
  docs/                          # legacy paper draft, experiment guide, research notes
  crates/
    sage-core/                   # deterministic protocol data model
    sage-manifest/               # migration manifest, certificates, signatures
    sage-consensus/              # PoA, HotStuff, quorum, pacemaker primitives
    sage-controller/             # SAGE migration state machine and invariants
    sage-sim/                    # deterministic event-driven simulator
    sage-stats/                  # confidence intervals and hypothesis tests
    sage-experiments/            # CLI experiment and artifact-generation binaries
    sage-store/                  # storage traits and in-memory backend
    sage-network/                # transport trait and in-memory transport
    sage-node/                   # early local multi-validator runtime
  results/                       # generated raw CSV, metadata, tables, figures
```

## Experiments

| Binary | Purpose | Typical output |
|--------|---------|----------------|
| `verify` | Artifact gate and invariant smoke checks | terminal PASS/FAIL summary |
| `run_rq1` | Migration cost against baselines | `results/raw/rq1_migration_cost.csv` |
| `run_rq2` | Partition safety / fork-rate trials | `results/raw/rq2_partition_safety.csv` |
| `run_rq3` | Kappa / shadow anchoring ablation | `results/raw/rq3_kappa_ablation.csv` |
| `run_rq4` | Rollback correctness / scaling inputs | `results/raw/rq4_rollback.csv` |
| `run_safety` | Adversarial partition safety | `results/raw/safety_partition.csv` |
| `run_termination` | Partition-duration termination | `results/raw/termination.csv` |
| `run_sensitivity` | Network-delay sensitivity | `results/raw/sensitivity.csv` |
| `run_manifest_tests` | Manifest negative tests and simulation checks | `results/raw/manifest_tests.csv` |
| `generate_tables` | Raw CSV to LaTeX snippets | `results/tables/*.tex` |
| `generate_figures` | Raw CSV to figure data | `results/figures/*` |

Run the standard artifact targets:

```bash
make all-lite   # cargo test + verify
make all        # test, verify, all experiments, tables, figures
```

## Multi-process testbed (real OS processes over loopback TCP)

Beyond the deterministic simulator, SAGE ships a real distributed testbed: N
`validator_proc` OS processes, each bound to its own loopback socket, crossing the
PoA→HotStuff cutover with no shared memory. The fork detector judges safety at the
finalized `state_root` level (not engine-tagged block hashes, which differ at the
cutover boundary because `engine_id` is bound into the hash). Set `SAGE_FORK_DEBUG=1`
for a per-height state_root/engine-group dump.

```bash
make testbed         # 3/3 partition: blind hardfork forks 5/5, quorum-gated SAGE 0/5
make msg-complexity  # n in {4..22} sweep -> results/raw/m4_message_complexity.csv (O(n^2) cost)
make byzantine       # one equivocator within f -> SAGE 0/5 forks (correct BFT safety)
```

| Experiment | What it shows | Output |
|------------|---------------|--------|
| `m3_partition_experiment.sh` | n-f cutover-quorum gate causally prevents the fork | terminal differential |
| `m4_message_complexity.sh` | empirical O(n²) broadcast cost (`per_round/n²` ~1.1–1.24) | `results/raw/m4_message_complexity.csv` |
| `m5_byzantine_equivocation.sh` | active Byzantine equivocation does NOT fork (quorum intersection) | `results/raw/m5_byzantine_equivocation.csv` |

The single-box liveness ceiling is ~n=22 (around 15/22 processes finish under the
30 s wall-clock valve, with no fork). Multi-region n=50 is scoped future work; the
loopback netem tier at n≈22 is the shipped bridge evidence.

## Verification Checkpoint

Last verified in this workspace on 2026-06-16:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo run -p sage-experiments --bin verify -- --config config/default.toml
cargo test -p sage-manifest --features real-crypto
```

Observed results:

| Metric | Value |
|--------|-------|
| Rust source files | 85 |
| Workspace crates | 10 |
| Experiment binaries | 11 (sage-experiments) + 4 (sage-node) |
| Config files | 6 |
| Default workspace tests | 116 passing (+2 `#[ignore]` on-demand) |
| Feature-gated real-crypto tests | 4 passing |
| Verify checks | 16 passing |
| Multi-process testbed scripts | m3 partition, m4 message-complexity, m5 Byzantine equivocation, m6 SOTA Cox-style baseline |
| Git metadata | unavailable in this checkout (`.git` missing) |

## Current Checkpoint

- Core protocol model, PoA, HotStuff, controller, simulator, experiment binaries,
  statistics helpers, and invariant checks are implemented and lint-clean.
- HotStuff includes timeout-certificate pacemaker primitives, but the simulator
  still uses deterministic external view synchronization for repeatable runs.
- `sage-store` and `sage-network` provide trait-based in-memory backends.
- `sage-node` can run a one-process local testnet, finalize PoA blocks, observe
  SAGE cutover, and finalize one post-cutover HotStuff block in smoke tests.
- The Ed25519 signature backend is available behind the `real-crypto` feature.

## Known Gaps

- `sage-node` now records local vote safety keys, rejects a conflicting vote
  after a cloned restart boundary, and restores committed height/state root from
  persisted in-memory snapshots. A full local-runtime restore can continue
  consensus to the target height; durable disk-backed recovery remains future work.
- Experiment metadata now uses SHA-256 and records config/tool context, but
  metadata write errors should still be propagated consistently by all binaries.
- CSV schema constants exist, but row structs should fully converge on common
  columns across every experiment binary.
- `verify` is stricter, but should still add generated table/figure validation
  and known-broken fixture checks.
- Table/figure generators now fail on missing required inputs unless
  `--allow-missing` is passed; they still need fuller table/figure coverage.
- Docs and paper tables must be regenerated from current Rust outputs before any
  artifact or paper submission.

## Documentation

- `docs/README.md` — documentation index and manuscript availability boundary
- `docs/paper/SAGE_ThaiCong_IEEE/main.tex` — current canonical manuscript
- `docs/paper.tex` — retained legacy manuscript draft
- `docs/ARTIFACT_EVALUATION.md` — scoped claim-to-evidence map
- `docs/RESPONSE_TO_REVIEWS.md` — historical response and current limitations
- `docs/guides/experiment_guide.md` — user-facing experiment guide
- `docs/research/` — research-backed design decisions

## License

MIT OR Apache-2.0
