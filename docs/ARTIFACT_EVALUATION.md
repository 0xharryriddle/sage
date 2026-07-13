# SAGE — Artifact Evaluation Guide

This is the reviewer-facing entry point for evaluating the SAGE artifact. It maps
each paper claim to a runnable command, states the toolchain, and is explicit
about which tiers are local/simulator, which are single-box multi-process, and
which require real hosts. Nothing here fabricates data; the harnesses that need
real hosts refuse to run without them.

## 1. Claims this artifact supports

| # | Paper claim | Evidence tier | Command | Output |
|---|---|---|---|---|
| C1 | Cross-boundary safety (no two correct validators conflict) | Proof + TLC + sim + real host | `make formal`; `cargo run -p sage-experiments --bin verify` | TLC 8/8; verify 18/18 |
| C2 | Decision uniqueness / no absolute reversion | Proof + TLC | `make formal` | 8/8, broken controls falsified |
| C3 | Observed fork differential (the n-f gate prevents the boundary fork) | Multi-process testbed | `make testbed` | hardfork 5/5, SAGE 0/5 |
| C4 | Faithful Cox's own 2f+1 gate STILL forks at the heterogeneous boundary | Loopback testbed | see §4 | Cox 20/20, SAGE 0/20 |
| C5 | Real-host safety differential (independent VMs, real TCP) | 6 cloud VMs | `bash scripts/deploy_multihost.sh` (real hosts) | SAGE 0/5, hardfork 5/5 |
| C6 | Real-host committed throughput parity vs Cox-style/hardfork | 6 cloud VMs | see §5 | ~1.65% spread (tie) |
| C7 | Validator-count scaling (flat cutover cost n=4..100) | Simulator | `cargo run -p sage-experiments --bin run_scaling` | `results/raw/scaling.csv` |
| C8 | Cert size O(n) real Ed25519 vs O(1) BLS analytic | Real crypto | `cargo run -p sage-experiments --features real-crypto --bin run_cert_sizes` | `results/raw/cert_sizes.csv` |

## 2. Toolchain (pinned)

- Rust stable (edition 2021); `rust-toolchain.toml` pins if present.
- OpenJDK 21 (bundled: `formal/tla2tools.jar`).
- Python 3 (stdlib only) for aggregators.
- Docker (optional) for a hermetic build.

## 3. Kick-the-tires (5 minutes, no special hardware)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --lib
cargo test --workspace
cargo run -p sage-experiments --bin verify   # 18 passed
make formal                                    # TLC 8/8
```

## 4. Faithful-Cox differential (loopback, deterministic, ~2 min)

Confirms C4: Cox's own 2f+1 checkpoint quorum still forks at the heterogeneous
boundary, while SAGE's n-f gate does not. Loopback `spawn_testbed`, no root, no
special hardware:

```bash
cargo build --release -p sage-node --bin validator_proc --bin spawn_testbed --features real-crypto
# Faithful Cox: expect observed_fork=true on each trial
for i in 1 2 3 4 5; do
  target/release/spawn_testbed --n 6 --f 1 --strategy coxfaithful \
    --max-height 8 --h-c 4 --partition 3 --max-secs 20 \
    --out results/raw/cox_trial_$i.csv >/dev/null 2>&1
  awk -F, 'NR==2{print "cox trial '"$i"': fork="$8}' results/raw/cox_trial_$i.csv
done
# SAGE control: expect observed_fork=false
for i in 1 2 3 4 5; do
  target/release/spawn_testbed --n 6 --f 1 --strategy sage \
    --max-height 8 --h-c 4 --partition 3 --max-secs 20 \
    --out results/raw/sage_trial_$i.csv >/dev/null 2>&1
  awk -F, 'NR==2{print "sage trial '"$i"': fork="$8}' results/raw/sage_trial_$i.csv
done
```

Consolidated reference result: `results/raw/coxfaithful_differential_20.csv`
(faithful Cox 20/20, SAGE 0/20; run `bash scripts/m8_coxfaithful_differential.sh 20`).
The earlier 5-trial run is retained at `results/raw/coxfaithful_differential.csv`.

## 5. Real-host tier (requires 6 hosts; optional)

Full provisioning runbook: `docs/guides/VPS_BENCHMARK_SETUP.md`. With a real
`config/hosts.txt` (one validator per line, `id user@ip region bind_ip:port`):

```bash
bash scripts/deploy_multihost.sh build
bash scripts/deploy_multihost.sh distribute
# throughput (no fault): set a synthetic workload for a real committed-TPS number
WORKLOAD_TXS=200 bash scripts/deploy_multihost.sh run sage 8
WORKLOAD_TXS=200 bash scripts/deploy_multihost.sh run hardfork 8
WORKLOAD_TXS=200 bash scripts/deploy_multihost.sh run coxstyle 8
```

Reference results captured on 6 Oracle Cloud VMs (same-region, real TCP):
`results/raw/multihost_tps_3arm.csv` (SAGE 777 / hardfork 790 / Cox-style 790 TPS),
`results/raw/multihost_n12_sage.csv` (n=12, 2 validators/host).

## 6. Honesty boundaries (read before citing any number)

- Simulator binaries (`run_scaling`, `run_overhead`, `run_rq*`) report simulator
  event-time microseconds, not wall-clock, unless a field is named `*_wall` or
  `committed_tps`.
- Real-host runs in `results/raw/multihost_*` are **same-region** (WAN latency, when
  present, was injected with `tc netem`). Absolute TPS reflects intra-region
  latency; cross-region geo throughput is future work.
- The BLS cert-size column is a computed analytic target, not an implemented
  pairing artifact; the Ed25519 column is measured under `--features real-crypto`.
- Faithful-Cox (`NodeStrategy::CoxFaithful`) models Cox's 2f+1 checkpoint quorum;
  it omits Cox's epoch-mismatch catch-up sub-protocol (a liveness feature that does
  not affect the boundary-safety differential). It is not a line-for-line port.
- Real-host faithful-Cox runs were liveness-flaky (post-cutover HotStuff with a
  3-node side has zero in-side fault tolerance) and are quarantined in
  `results/raw/inconclusive/`; the cited faithful-Cox result is the deterministic
  loopback differential.

## 7. Hermetic Docker build

```bash
docker build -t sage-artifact -f Dockerfile .
docker run --rm sage-artifact    # runs the core verification gate
```
