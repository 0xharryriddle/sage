# SAGE Experiment Guide

This guide describes how to reproduce the current Rust-based SAGE artifact. It
replaces the earlier Python-era workflow: experiments are now Cargo binaries,
configuration is TOML, and outputs are CSV/JSON files under `results/`.

## 1. Artifact Scope

SAGE is evaluated primarily as a deterministic protocol artifact. The simulator
executes block finalization, shadow validation, migration control, manifest
checks, rollback logic, adversarial partitions, and strategy comparisons. The
local node crate provides early one-process runtime evidence, but production
networking and durable crash recovery are still future work.

Current claim boundary:

- Supported: protocol-level safety/liveness evidence from deterministic Rust
  simulation, executable invariants, manifest negative tests, and statistical
  summaries.
- Partially supported: local multi-validator runtime (`sage-node`) with PoA
  finalization, SAGE cutover observation, one post-cutover HotStuff-finalized
  block, store-backed double-vote rejection, and in-memory restart/continue
  recovery.
- Not yet fully supported: production deployment claims, real distributed
  networking, durable disk-backed restart recovery, and geo-distributed
  performance claims.

## 2. Environment

Required:

- Rust toolchain with Cargo (edition 2021 workspace).
- A POSIX shell for `make` targets.
- No external dataset is needed; workloads are synthetic and seeded.
- Optional: LaTeX toolchain if compiling `docs/paper.tex` with generated table
  snippets.

Recommended verification before experiments:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo run -p sage-experiments --bin verify -- --config config/default.toml
```

Feature-gated real-crypto smoke test:

```bash
cargo test -p sage-manifest --features real-crypto
```

## 3. Repository Layout

```text
config/
  default.toml       # small deterministic smoke config
  rq1.toml           # migration-cost experiment config
  rq2.toml           # partition safety / fork-rate config
  rq3.toml           # kappa / shadow anchoring config
  rq4.toml           # rollback / scaling config
  safety.toml        # adversarial safety config
crates/
  sage-core/         # protocol data model
  sage-consensus/    # PoA, HotStuff, quorum, pacemaker primitives
  sage-controller/   # SAGE state machine and invariants
  sage-sim/          # deterministic simulator
  sage-experiments/  # experiment CLI binaries
  sage-stats/        # CIs and statistical tests
  sage-manifest/     # manifests, certificates, signatures
  sage-store/        # storage traits and in-memory backend
  sage-network/      # transport trait and in-memory transport
  sage-node/         # early local runtime with restart/recovery smoke tests
results/
  raw/               # experiment CSVs
  metadata/          # per-run metadata JSON files
  tables/            # generated LaTeX snippets
  figures/           # generated figure data
```

## 4. Experiment Binaries

| Binary | Purpose | Output |
|--------|---------|--------|
| `verify` | Fast artifact gate and invariant checks | terminal PASS/FAIL |
| `run_rq1` | Migration cost against baselines | `results/raw/rq1_migration_cost.csv` |
| `run_rq2` | Partition safety / fork-rate trials | `results/raw/rq2_partition_safety.csv` |
| `run_rq3` | Kappa / shadow anchoring ablation | `results/raw/rq3_kappa_ablation.csv` |
| `run_rq4` | Rollback correctness inputs | `results/raw/rq4_rollback.csv` |
| `run_safety` | Adversarial partition safety | `results/raw/safety_partition.csv` |
| `run_termination` | Partition-duration termination | `results/raw/termination.csv` |
| `run_sensitivity` | Network-delay sensitivity | `results/raw/sensitivity.csv` |
| `run_manifest_tests` | Manifest negative tests | `results/raw/manifest_tests.csv` |
| `generate_tables` | CSV to LaTeX tables | `results/tables/*.tex` |
| `generate_figures` | CSV to figure data | `results/figures/*` |

## 5. Common Commands

Fast checkpoint:

```bash
make all-lite
```

Full artifact run:

```bash
make all
```

Individual runs:

```bash
cargo run -p sage-experiments --bin run_rq1 -- --config config/rq1.toml --seeds 0,1,2
cargo run -p sage-experiments --bin run_rq2 -- --config config/rq2.toml --seeds 0,1,2
cargo run -p sage-experiments --bin run_rq3 -- --config config/rq3.toml --kappa 1,2,4,8
cargo run -p sage-experiments --bin run_rq4 -- --config config/rq4.toml --seeds 0,1,2
cargo run -p sage-experiments --bin run_safety -- --config config/safety.toml --trials 30
cargo run -p sage-experiments --bin run_termination -- --config config/rq2.toml --durations 2,4,8 --trials 5
cargo run -p sage-experiments --bin run_sensitivity -- --config config/rq1.toml --delays 40000,80000 --trials 5
cargo run -p sage-experiments --bin run_manifest_tests -- --out-dir results/raw
```

Regenerate tables and figures after raw CSVs exist:

```bash
cargo run -p sage-experiments --bin generate_tables -- --raw-dir results/raw --out-dir results/tables
cargo run -p sage-experiments --bin generate_figures -- --raw-dir results/raw --out-dir results/figures
```

## 6. Research Questions and Mapping

| RQ | Question | Primary binary |
|----|----------|----------------|
| RQ1 | Migration cost and finality continuity versus baselines | `run_rq1` |
| RQ2 | Partition safety and fork behavior | `run_rq2`, `run_safety` |
| RQ3 | Contribution of shadow anchoring / kappa | `run_rq3` |
| RQ4 | Rollback correctness and scaling inputs | `run_rq4` |
| E6 | Termination under partition duration | `run_termination` |
| E7 | Sensitivity to network delay | `run_sensitivity` |
| Manifest checks | Rejection of invalid/stale/replayed manifests | `run_manifest_tests` |

## 7. Metrics

Common metrics include:

- `finalized_blocks` and `max_finalized_height`
- `migration_success`
- `safety_violation`
- `max_finalization_gap_micros` (RQ1): observed service interruption, measured
  as the largest gap (simulated microseconds) between the first-finalization
  timestamps of consecutive heights. A stop-the-world upgrade halts during its
  snapshot/restart window (config `simulation.stw_downtime_micros`) and shows a
  large gap; zero-halt strategies stay at normal block cadence. Measured from
  recorded timestamps, NOT derived from the strategy flag.
- `disjoint_quorum_windows` (RQ2/safety): count of heights where a non-CutCert
  cutover left BOTH partition sides simultaneously holding >= BFT quorum (2f+1),
  i.e. a fork was *structurally possible*. This is an EXPOSURE measure and is
  never treated as a safety violation; `safety_violation` only fires on a
  genuinely observed conflicting commit.
- strategy label and seed
- run status (`completed`, `error:*`, or experiment-specific status)
- confidence intervals from `sage-stats` where applicable

Metadata JSON records stable SHA-256 configuration hashes, config/tool context,
seed, strategy, run status, and artifact-version fields. This is sufficient for
artifact-level provenance, although this checkout has no `.git` directory, so
commit hashes are recorded as unavailable until repository metadata is restored.

## 8. Expected Interpretation

- SAGE should complete no-fault migration without safety violations in configured
  deterministic runs.
- Reconfig-only should not report a successful protocol swap.
- HardFork and SageBlind are baseline-risk strategies used to expose why global
  cutover coordination matters.
- The RQ2/safety exposure metric makes this concrete and reproducible: under a
  partition that straddles the cutover height with a balanced split (requires
  `n >= 4f+2` so both sides reach quorum), HardFork and SageBlind are exposed to
  a disjoint-quorum fork window in 100% of trials, while SAGE's CutCert gate
  keeps exposure at 0%. The *observed* fork rate is 0 for all strategies in the
  deterministic `sage-sim` simulator because it elects a single global proposer
  per height and cannot drive two concurrent leaders.
- Observed forks ARE now demonstrated outside the serialized simulator: the
  `sage-node` integration test `tests/observed_fork.rs` drives the real HotStuff
  engines in two partition-isolated quorum groups (n=6, f=1, quorum=3). Each
  group independently reaches quorum and finalizes a different block at the same
  height, and the executable fork detector (`sage_node::fork_detector`) reports
  a real observed fork. A broken-control test proves the detector fires only on
  an actual conflict; an unpartitioned control produces no fork.
- Stop-the-world is expected to preserve safety but represent a downtime
  baseline: RQ1 reports its observed `max_finalization_gap_micros` (~8 ms with the
  default `stw_downtime_micros`) versus ~0.12 ms for zero-halt strategies.
- Each baseline fails on a DIFFERENT axis, so no single scalar ranks them:
  stop-the-world on downtime, HardFork/SageBlind on fork exposure (RQ2),
  reconfig-only on protocol swap (`protocol_swap_success=false`). RQ1 emits
  `rq1_stats.csv` with SAGE-vs-baseline Mann-Whitney U tests, rank-biserial
  effect sizes, and Holm-Bonferroni family-wise decisions on the downtime metric.
  Read it with care: SAGE-vs-StopTheWorld is significant with maximal effect
  (rank-biserial +1.0), SAGE-vs-HardFork is correctly NOT significant (both
  zero-halt), and SAGE-vs-ReconfigOnly is significant by p-value but the absolute
  difference is ~6 us -- a statistical-vs-practical-significance caution, since
  reconfig-only's real failure is the absent protocol swap, not downtime.
- `verify` is a fast artifact gate that checks configuration loading,
  deterministic reproducibility, key strategy behavior, invariant execution,
  metadata hashing, schema constants, and existing raw CSV headers.

## 9. Current Limitations

- Simulator HotStuff uses deterministic external view synchronization for
  repeatable experiments even though consensus pacemaker primitives exist.
- Local `sage-node` uses an in-memory transport/store and is still a one-process
  runtime, not a production distributed node.
- Durable disk-backed recovery remains future work; current restart evidence uses
  cloned in-memory stores to validate state restoration and safety keys.
- `generate_tables` and `generate_figures` intentionally fail on missing required
  inputs unless `--allow-missing` is supplied.
- The serialized simulator cannot realize an observed cross-partition fork (one
  global proposer per height), so RQ2/safety differentiate strategies via the
  disjoint-quorum *exposure* metric rather than an observed fork rate.
- The checkout used for the latest scan had no `.git` directory, so generated
  metadata cannot include commit hashes until repository metadata is restored.

## 10. Paper Update Checklist

Before submitting or circulating the paper:

1. Run `make all` from a clean checkout.
2. Confirm all raw CSVs and metadata files are regenerated.
3. Run table/figure generation.
4. Update `docs/paper.tex` from generated outputs only.
5. Ensure every claim in the paper matches the artifact claim boundary above.
6. Clearly separate simulator evidence from local-runtime and future production
   claims.
