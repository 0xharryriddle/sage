# REPRODUCE.md

One-command reproduction map for SAGE artifacts. Every row ties a paper claim or
results file to the exact command that regenerates it. Seeds are fixed in the
experiment binaries / configs; runs are deterministic unless noted.

## Toolchain (pinned)

- Rust: stable (edition 2021). `rustup show` to confirm; `rust-toolchain.toml` pins it if present.
- Java: OpenJDK 21 (bundled `formal/` uses `formal/tla2tools.jar`).
- Python 3 (stdlib only) for the multi-host aggregator.

## Core verification gate

```bash
cargo fmt --all -- --check
cargo clippy --workspace --lib            # robustness gate: no unwrap/expect on runtime paths
cargo test --workspace                    # all unit/property/integration tests
cargo run -p sage-experiments --bin verify
```

All four must pass (zero failures). The clippy step enforces the P1-D robustness
gate: `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]` on
`sage-node`, `sage-consensus`, `sage-controller`.

## Artifact -> command map

| Artifact / claim | Command | Output |
|---|---|---|
| Validator-count scale study (cutover cost flat n=4..100) | `cargo run -p sage-experiments --bin run_scaling -- --config config/rq1.toml --validators 4,7,10,13,16,20,25,31,50,100 --trials 20` | `results/raw/scaling.csv`, `results/figures/scaling.csv` |
| Cutover-certificate size: real Ed25519 O(n) vs BLS O(1) | `cargo run -p sage-experiments --features real-crypto --bin run_cert_sizes` | `results/raw/cert_sizes.csv` |
| Assumption 3 admissibility study (A=89%, modeled) | `cargo run -p sage-experiments --bin run_assumption3` | `results/raw/assumption3_admissibility.csv`, `results/raw/assumption3_summary.csv` |
| Adversarial negative-controls campaign | `bash scripts/negative_controls.sh 5 6 4` | `results/raw/negative_controls.csv` (faithful 0-fork, broken control forks) |
| Dual-run overhead microbenchmark (local) | `cargo run -p sage-experiments --bin run_overhead -- --trials 5 --txs-per-block 10,25,50` | `results/raw/dual_run_overhead.csv`, `results/figures/dual_run_overhead_summary.csv` |
| Multi-process observed-fork differential | `make testbed` (or `bash scripts/m3_partition_experiment.sh`) | hardfork forks 5/5, SAGE 0/5 |
| Message-complexity O(n^2) sweep | `make msg-complexity` | `results/raw/m4_message_complexity.csv` |
| Byzantine equivocation | `make byzantine` | `results/raw/m5_byzantine_equivocation.csv` |
| SOTA (Cox-style) differential | `bash scripts/m6_sota_baseline_differential.sh 10 6 4` | `results/raw/m6_sota_baseline_differential.csv` |
| Local WAN emulation (single-box, labeled) | `bash scripts/local_wan_emulation.sh 6 3` | `results/raw/local_wan_emulation.csv` |
| Kernel-netns testbed (real net stacks + real tc netem, on-box) | `sudo bash scripts/netns_testbed.sh 4 50 10 0.1` | `results/raw/netns_testbed.csv` |
| Multi-HOST geo deployment (requires real hosts; full setup in `docs/guides/VPS_BENCHMARK_SETUP.md`) | `bash scripts/deploy_multihost.sh build && bash scripts/deploy_multihost.sh distribute && bash scripts/deploy_multihost.sh run sage 8` | `results/raw/multihost_sage.csv` |
| Bounded model check (safety + rollback + replay-ctx fail-closed) | `make formal` (or `bash formal/run_tlc.sh`) | 8/8 TLC checks pass |
| Provenance (machine-checked) | `make provenance` (or `python3 scripts/check_figure_provenance.py`) | scaling-figure coordinates, cert-size series (when present), and the RQ1 downtime table values match their named source CSVs. Other table values are traceable to their named CSVs but not all machine-checked. |

## Honesty boundary (read before citing)

- `run_scaling`, `run_overhead`, `run_cert_sizes` and all `run_rq*` binaries are
  LOCAL / SIMULATOR-capacity measurements. Latencies are simulator event-time
  microseconds unless explicitly wall-clock.
- `scripts/local_wan_emulation.sh` is a SINGLE-BOX netem-style emulation. It is NOT
  a geo-distributed deployment and abstracts away host scheduler contention, clock
  drift, and disk I/O.
- `scripts/deploy_multihost.sh` is the ONLY path that produces real multi-host
  numbers. It refuses to run without a real `config/hosts.txt` and never fabricates
  geo data. Current committed real-host CSVs retain trial verdicts and aggregate
  roots/latency/TPS summaries; the first same-region fork-differential CSV records
  that per-trial temporary directories were not retained, so independent auditors
  should reproduce that tier with the command above if they need per-validator logs.
- The BLS column in `cert_sizes.csv` is a COMPUTED analytic target (48-byte aggregate
  + ceil(n/8) bitmap), not an implemented pairing artifact. The Ed25519 column is
  measured from real signatures under `--features real-crypto`.

## Docker (hermetic build + test)

```bash
docker build -t sage-artifact -f Dockerfile .
docker run --rm sage-artifact            # runs the core verification gate
```
