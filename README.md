# SAGE — Shadow-Anchored Graceful Evolution

SAGE migrates a live permissioned blockchain from one consensus engine to another
without halting block production. This repository is a Rust protocol artifact
covering deterministic simulation, experiment generation, manifest validation,
and an early local multi-validator runtime.

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

A stronger control, **faithful Cox** (`NodeStrategy::CoxFaithful`), gates on Cox's *actual* 2f+1
BFT checkpoint quorum (omitting only Cox's epoch-mismatch catch-up sub-protocol). Because
2(2f+1) <= n once n > 3f+1, each side of a balanced 3/3 partition independently reaches Cox's
2f+1=3 checkpoint quorum and switches, so even Cox's real gate forks (5/5) at the heterogeneous
boundary, while SAGE's n-f=5 gate does not (0/5). This isolates the n-f *threshold* — not merely
"having a gate" — as the contribution (`results/raw/coxfaithful_differential.csv`).

## Real-host throughput (6 independent cloud VMs, real TCP)

A fair same-conditions 3-arm run (identical n=6, f=1, 200 txs/block, matched pacemaker) measures
committed throughput and finalization-latency distributions on six independent cloud hosts:

| Strategy | Committed TPS | vs SAGE | Cutover handoff gap |
|----------|---------------|---------|---------------------|
| SAGE | 777 | — | 10.6 ms |
| Hard fork (blind) | 790 | +1.62% | 10.8 ms |
| Cox-style switch | 790 | +1.65% | 10.3 ms |

The 1.65% spread is a statistical tie: SAGE's dual-run gate imposes no measurable steady-state
throughput penalty. The cutover handoff gap (~10.5 ms) is an order of magnitude below the ~500 ms
steady-state block cadence — the migration perturbation is smaller than an ordinary inter-block
interval (the empirical realization of the T_bdy liveness bound). Scaled to n=12 (2 validators/host):
764 TPS, migrates, zero fork. Same-region; cross-region geo throughput remains future work.
See `results/raw/multihost_tps_3arm.csv`, `results/raw/multihost_n12_sage.csv`.

## Workspace Structure

```text
sage/
  Cargo.toml                     # workspace root
  Makefile                       # build/test/experiment targets
  config/                        # default, rq1-rq4, safety TOML configs
  docs/                          # paper draft, plans, experiment guide, research notes
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

Last verified in this workspace on 2026-07-05:

```bash
cargo fmt --all --check
cargo clippy --workspace --lib
cargo test --workspace
cargo run -p sage-experiments --bin verify
cargo test -p sage-manifest --features real-crypto
make formal        # TLA+/TLC 8/8
make provenance    # plotted paper coords == source CSVs
```

Observed results:

| Metric | Value |
|--------|-------|
| Rust source files | 88 |
| Workspace crates | 10 |
| Experiment binaries | 15 (sage-experiments) + 4 (sage-node) |
| Config files | 6 |
| Default workspace tests | 127 passing (+2 `#[ignore]` on-demand) |
| Feature-gated real-crypto tests | 3 passing |
| Verify checks | 17 passing |
| Formal (TLA+/TLC) | 8/8 (safety n∈{4,7,10} + rollback; broken controls falsified) |
| Multi-process testbed scripts | m3 partition, m4 message-complexity, m5 Byzantine equivocation, m6 SOTA Cox-style baseline |
| Real-host tier | 6 cloud VMs: fair 3-arm TPS (SAGE 777 / hardfork 790 / Cox-style 790), n=12 scale, faithful-Cox 5/5 vs SAGE 0/5 |

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

- `docs/paper/paper.tex` — journal paper draft (24 pp)
- `docs/ARTIFACT_EVALUATION.md` — reviewer-facing artifact-evaluation guide (claim→command map)
- `docs/COVER_LETTER.md` — response-to-reviews cover letter
- `docs/RESPONSE_TO_REVIEWS.md` — point-by-point review-response mapping
- `docs/EXPERIMENT_FAIRNESS.md` — held-equal-knob fairness contract for cross-strategy comparison
- `REPRODUCE.md` — artifact→command reproduction map
- `docs/guides/experiment_guide.md` — user-facing experiment guide
- `docs/research/` — research-backed design decisions
- `graphify-out/GRAPH_REPORT.md` — knowledge-graph audit report (read before architecture work)

## License

MIT OR Apache-2.0
