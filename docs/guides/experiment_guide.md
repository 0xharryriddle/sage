# SAGE Experiment Guide

This guide describes the Rust experiment and artifact-generation commands that
are present on `main`. Experiments create fresh CSV/JSON outputs; they do not
reconstruct the paper's retained campaigns merely because a config or seed
label matches.

## 1. Artifact Scope

SAGE is a conditional certified-boundary design. Source finalization continues
during shadow preparation, followed by a deliberate certification pause before
target activation. The simulator exercises a partial protocol path and records
event-time metrics; it does not execute the complete committee-wide
manifest/fence/abort/seal transport or measure client-visible interruption.

Current claim boundary:

- Supported within bounded scope: deterministic simulation behavior,
  executable invariants, selected manifest-negative checks, bounded formal
  design models, and synthetic CSV transformations.
- Partially supported: constructed loopback multi-process controls for the
  `CutCert`-gated PoA-to-HotStuff subset, with local fence/restart behavior.
- Not established: complete networked certificate execution, persistent Rule 1
  enforcement across every crash point, certified abort/seal end to end,
  authenticated placement, physical geo-WAN behavior, client receipt
  reconciliation, or production performance.

## 2. Environment

Required:

- Rust toolchain with Cargo (edition 2021 workspace).
- A POSIX shell for `make` targets.
- No external dataset is needed; workloads are synthetic and seeded.
- Optional: a LaTeX toolchain for the canonical manuscript rebuild under
  `docs/paper/SAGE_ThaiCong_IEEE/`.

Recommended verification before experiments:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --no-fail-fast
cargo run -p sage-experiments --bin verify
```

`verify` loads the checked-in configurations itself; it accepts no `--config`
argument. A passing verifier is not validation of retained CSV rows or the
complete handoff.

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
| `run_rq4` | Simulator-derived abort-window/admissibility proxy; does not execute certified rollback | `results/raw/rq4_rollback.csv` |
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
| RQ4 | Simulator-derived abort-window/admissibility proxy and scaling inputs | `run_rq4` |
| E6 | Termination under partition duration | `run_termination` |
| E7 | Sensitivity to network delay | `run_sensitivity` |
| Manifest checks | Rejection of invalid/stale/replayed manifests | `run_manifest_tests` |

## 7. Metrics

Common metrics include:

- `finalized_blocks` and `max_finalized_height`
- `migration_success`
- `safety_violation`
- `max_finalization_gap_micros` (RQ1): the largest simulated-time gap
  between consecutive finalized heights. The stop-the-world arm includes a
  configured maintenance halt. Other arms continue the partial simulator
  pipeline, but this does **not** measure SAGE's complete certification pause
  or client downtime.
- `disjoint_quorum_windows` (RQ2/safety): count of heights where a non-CutCert
  cutover left BOTH partition sides simultaneously holding >= BFT quorum (2f+1),
  i.e. a fork was *structurally possible*. This is an EXPOSURE measure and is
  never treated as a safety violation; `safety_violation` only fires on a
  genuinely observed conflicting commit.
- strategy label and seed
- run status (`completed`, `error:*`, or experiment-specific status)
- confidence intervals from `sage-stats` where applicable

Metadata JSON records stable SHA-256 configuration hashes, config/tool context,
seed, strategy, run status, and artifact-version fields. Git commit availability
depends on running inside a Git checkout; metadata alone is not a complete
execution-environment or evidence attestation.

## 8. Expected Interpretation

- SAGE should complete configured no-fault simulator runs without recorded
  safety violations; this is not a full-protocol execution claim.
- Reconfig-only is a negative control and should not report a protocol swap.
- HardFork and SageBlind are internal risk controls, not external system
  implementations.
- `disjoint_quorum_windows` is exposure, not an observed safety violation.
  The serialized simulator has one proposer per height and cannot realize a
  concurrent two-leader fork.
- Process/testbed observations are constructed controls on a partial path.
  Interpret their exact schedule, threshold and gate separately; they neither
  authenticate deployment geography nor prove `n-f` uniquely necessary.
- RQ1 is an inadmissible, quorum-confounded partial-path comparison. Its gaps
  are simulator event time; the internal `CoxStyle` arm is not Cox, and the
  configured STW halt dominates that contrast. No scalar ranks all strategies.
- `run_rq4` computes `rollback_occurred` and latency from the configured window
  and simulator outcome. It is an admissibility/rollback proxy, not execution
  of `AbortCert`, state replay, or certified rollback.
- `verify` is a smoke/invariant/schema gate, not a comprehensive evidence or
  paper-submission gate.

## 9. Current Limitations

- The process path does not execute committee-wide `ManifestCert`, `FenceCert`,
  `AbortCert`, and `SealCert` transport end to end.
- The certification pause and client-visible service interruption are not
  measured on the complete path.
- Receipt revocation/reconciliation and compensation for provisional external
  effects remain outside the implementation.
- Configured endpoints do not attest machine identity, placement, impairment,
  or physical geo-WAN isolation.
- The simulator cannot realize a concurrent cross-partition fork.
- Bounded formal checks are not an implementation refinement proof.
- Generated tables/figures summarize supplied rows; they do not authenticate
  those rows as admitted evidence.

## 10. Paper and Artifact Checklist

Before circulating results:

1. Run workspace checks and report actual outcomes; do not copy historical
   counts.
2. Write fresh experiment outputs to a new directory. Do not overwrite the
   paper's hash-pinned retained inputs.
3. Use `docs/paper/SAGE_ThaiCong_IEEE/rebuild.py --check-data` for paper-input
   identity and `--verify-rebuild` for the presentation rebuild.
4. Keep simulator, process, configured-endpoint, formal, and presentation
   evidence tiers distinct.
5. Match every claim to `docs/ARTIFACT_EVALUATION.md` and disclose unresolved
   full-protocol, comparator, placement, and client-semantics gaps.
