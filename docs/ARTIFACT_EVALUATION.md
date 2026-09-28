# SAGE — Artifact Evaluation Guide

This is the reviewer-facing entry point for evaluating the SAGE artifact. It maps
each paper claim to a runnable command, states the toolchain, and is explicit
about which tiers are local/simulator, which are single-box multi-process, and
which are retained configured-endpoint records. The retained campaign does not
attest endpoint provisioning, placement, or physical network isolation.

The canonical manuscript is `docs/paper/SAGE_ThaiCong_IEEE/main.tex`. This guide
does not treat superseded paper generations or submission bundles as evidence.

## Current manuscript repair boundary

The report-review revision distinguishes durable Rule 1 source exclusion from
FenceCert's policy-bound retirement evidence. The bounded fence-only model does
not model that readiness lock, and the process path does not exercise complete
manifest/fence/abort/seal transport. All process results use the crash-only source
profile; they do not empirically validate the strongest cross-domain Byzantine
source model. RQ1 remains an inadmissible, quorum-confounded partial-path
microbenchmark. No external switching implementation has been benchmarked here.

`make paper` rebuilds all six vector figures, four numerical tables and the manuscript
from eight pinned retained CSVs and two pinned provenance notes, then runs paper gates
before publishing. `make paper-verify` tests the generator and compares 22 generated
products from two isolated builds. `TABLE_PROVENANCE.json` names each table's input,
formula, rounding and retained-trial versus aggregate-only evidence boundary.
These commands do not rerun experiments or reconstruct missing trial records.
The relocatable paper bundle described in `REPRODUCE.md` packages the presentation
test/checker dependencies as well as the retained plot/table inputs. Its same-host
run compares all 22 products with the source checkout's expected hashes; no public
checkout or hermetic toolchain is implied by that comparison.
See `REPRODUCE.md` for the executable workflow and the tracked
`docs/RESPONSE_TO_REVIEWS.md` for a historical response with current-scope warnings.
The detailed item disposition is operator-local under gitignored `docs/reviews/`
and is not available in a public clone. Rows below describe retained evidence
and available commands, not a fresh execution receipt or comprehensive seal.

## 1. Claims this artifact supports

| # | Paper claim | Evidence tier | Command | Output |
|---|---|---|---|---|
| C1 | Conditional forward-handoff safety; source exclusion via durable readiness locking or the separate fence-only contract | Size-parametric argument and bounded fence-only TLC; not a runtime refinement proof or end-to-end Rule 1 crash campaign | `make formal`; `cargo run -p sage-experiments --bin verify` | Retained TLC/verify results within their abstractions; faithful partial-delivery n=7 remains inconclusive |
| C2 | Decision uniqueness / no absolute reversion | Proof + bounded TLC abstractions | `make formal` | Retained 20 expected checks include faithful finite cases and falsified broken controls; faithful partial-delivery `n=7` remains inconclusive, and no implementation refinement follows |
| C3 | Constructed balanced-partition differential under the tested gates | Multi-process testbed | `make testbed` | hardfork 5/5, SAGE 0/5 |
| C4 | Unsafe under-quorum control (`n=6`, `f=1`, `q=3`) forks | Loopback testbed | see §4 | unsafe control 20/20, SAGE 0/20 |
| C5 | Retained configured-endpoint finalized-prefix differential (terminality, activation, placement, and isolation unattested) | Exact retained campaign closure `20260713T133617Z` | `python3 scripts/verify_safety_forest_campaign.py`; `python3 scripts/generate_evaluation_plot_data.py --safety-only`; `python3 scripts/check_figure_provenance.py` | 10 seed-bound observations: SAGE 0/5, blind hard-fork 5/5 |
| C6 | Configured-endpoint crash recovery, including a gap spanning the boundary; partial path, not authenticated physical or same-region deployment evidence | `results/raw/m10_crash_recovery.{README.md,csv}` (h_c=4, gap ABOVE boundary); `results/raw/m11_boundary_spanning_recovery.{README.md,csv}` and `results/raw/m11b_atomic_transition_recovery.{README.md,csv}` (h_c=260, gap SPANS boundary) | `python3 scripts/verify_crash_recovery.py <evidence-dir> --recovered-id 5 --expect-validators 6`; `python3 scripts/verify_crash_recovery.py --self-test` | Retained m10: 4/5 caught up; m11: 3/5; m11b: 5/5. Binary generations differ; not a causal estimate of the atomic-write change |
| C7 | Simulator migration-completion robustness for n=4..100 | Simulator | `cargo run -p sage-experiments --bin run_scaling` | `results/raw/scaling.csv` |
| C8 | Certificate size and local verification cost for real Ed25519 shares | Real crypto | see `REPRODUCE.md` | Canonical paper timing input is the retained sealed `results/raw/local_refresh/scientific_refresh_20260714T170701Z/attempts/remaining_release/cert_timing.csv`; fresh top-level output is a different run |

## 2. Toolchain requirements

- Rust pinned by `rust-toolchain.toml` (channel `1.98.0` with `rustfmt` and `clippy`), so every `cargo`
  command below resolves the same compiler. Changing the channel requires re-qualifying the workspace
  before publishing new numbers; retained pre-pin results were built under cargo 1.97.1.
- OpenJDK 21 (bundled: `formal/tla2tools.jar`).
- Python 3 (stdlib only) for aggregators.
- Docker (optional) for a containerized build; no hermeticity claim is made.

## 3. Local verification (no special hardware; duration depends on workspace)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --no-fail-fast
cargo run -p sage-experiments --bin verify   # inspect the current result; no fixed pass count implied
```

Registered RED selectors must be reported separately, not repaired to manufacture
a green workspace. The retained comprehensive manifest can be inspected with
`python3 scripts/final_claim_evidence.py verify`, but later source/document changes
invalidate it; it is not the presentation-build manifest and is not silently resealed.

`make formal` is not part of the quick path: the retained 20 TLC verdicts include
broken controls, and one `SageFenceSplit_n7` run historically took about 10 minutes.
Run the bounded suite separately; its outcome must be interpreted for the current inputs.

## 4. Unsafe-threshold differential (loopback, deterministic, ~2 min)

Confirms C4 only within the constructed configuration: an unsafe `q=3` control
forks under `n=6`, `f=1`, while SAGE's configured `q=n-f=5` arm does not. For
this committee, correct-signer intersection requires `2q-n>f`, hence `q>=4`;
the `q=3` target is unsafe independently of migration. This does not execute,
reimplement, or falsify Cox, and it does not show that `n-f` is uniquely
necessary because the safe `q=4` arm must be considered separately.

The dedicated endpoint-control campaign runs all three thresholds. Under the
balanced partition, `q=5` and `q=4` each report `0/20` conflicts while the unsafe
`q=3` arm reports `20/20`; without the fault, both safe arms migrate `20/20`.
Those finite observations show that `n-f` is conservative within the safe
interval, not uniquely necessary.

For a five-trial kick-the-tires subset, use the loopback `spawn_testbed` path
below (no root or special hardware):

```bash
cargo build --release -p sage-node --bin validator_proc --bin spawn_testbed --features real-crypto
# Historical CLI name; semantically this is the unsafe q=3 control.
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
(unsafe q=3 control 20/20, SAGE q=5 control 0/20; run
`bash scripts/m8_coxfaithful_differential.sh 20`). The historical filename and
CLI strategy are retained for compatibility, not as a fidelity claim.
The earlier 5-trial run is retained at `results/raw/coxfaithful_differential.csv`.

## 5. Configured-endpoint tier (six reachable endpoints required; optional)

Full operator runbook: `docs/guides/VPS_BENCHMARK_SETUP.md`. With a reachable
`config/hosts.txt` (one validator per line, `id user@ip label bind_ip:port`):

```bash
bash scripts/deploy_multihost.sh build
bash scripts/deploy_multihost.sh distribute
# Validator-minted commit-rate microbenchmark (no external clients):
WORKLOAD_TXS=200 bash scripts/deploy_multihost.sh run sage 8
WORKLOAD_TXS=200 bash scripts/deploy_multihost.sh run hardfork 8
WORKLOAD_TXS=200 bash scripts/deploy_multihost.sh run coxstyle 8
```

The paper's retained configured-endpoint evidence is the safety differential, not
a throughput comparison. Its complete admitted evidence root is
`results/raw/multihost_evidence/20260713T133617Z/`. The archive records configured
private endpoints, launch arguments, logs, and validator results, but does not
attest provisioning, cloud or region placement, independent machines, physical
network isolation, or injected RTT. Validator-minted commit-rate files remain
diagnostic and are not used to claim client-observed throughput or overhead
equivalence.

## 5b. Crash-recovery tier (six reachable endpoints required; optional)

Requires a populated `config/hosts.txt` and SSH reachability. These runners spawn
real remote processes; they refuse to fabricate a result without hosts.

```bash
# Post-boundary gap (the recovered node's gap lies entirely above the cutover):
bash scripts/m10_crash_recovery.sh 42

# BOUNDARY-SPANNING gap (h_c inside the crash window) — the case that exercises
# adopting a legacy prefix up to h_c, reserving that height, and crossing the
# n-f gate during catch-up:
H_D=2 H_C=260 H_R=2000 CRASH_AFTER=20 RESTART_DELAY=6 MAX_SECS=120 \
  bash scripts/m10_crash_recovery.sh 90
```

The runner prints a descriptive summary, but the BINDING verdict is the verifier,
never the runner's stdout:

```bash
python3 scripts/verify_crash_recovery.py \
  results/raw/multihost_evidence/<campaign-dir> --recovered-id 5 --expect-validators 6
```

`--expect-validators` matters: without it, a partially collected run (a truncated
scp) would be verified only over the files that happened to arrive. Runs recorded
after 2026-07-25 carry `validators=` in `metadata.txt` so the verifier can prove
collection completeness on its own.

Each run's `metadata.txt` records the binary sha256, `h_d`/`h_c`/`h_r`, crash
timing, git commit, and whether the working tree was dirty. Do not compare results
across runs whose binary hashes differ without saying so.

## 6. Honesty boundaries (read before citing any number)

- Simulator binaries (`run_scaling`, `run_overhead`, `run_rq*`) report simulator
  event-time microseconds, not wall-clock, unless a field is named `*_wall` or
  `committed_tps`.
- `committed_tps` in the process testbed counts deterministic, height-derived,
  validator-minted, state-neutral transactions. There is no external request
  ingress or client response in this harness, so this is a commit-rate
  microbenchmark, not client-observed end-to-end blockchain throughput.
- The retained campaign must be cited only as configured-endpoint evidence. Its
  host labels and requested reachability schedule are not infrastructure or
  physical-network attestations. Cross-region throughput and finality latency are
  future work.
- The BLS cert-size column is a computed analytic target, not an implemented
  pairing artifact; the Ed25519 column is measured under `--features real-crypto`.
- `NodeStrategy::CoxFaithful` is a misleading historical code name for the unsafe
  `q=3` threshold control. It does not model Cox within Cox's valid system
  configuration and omits Cox's configuration, checkpoint-chain, initialization,
  and recovery semantics.
- Configured-endpoint unsafe-threshold runs were liveness-flaky (post-cutover HotStuff with a
  3-node side has zero in-side fault tolerance) and are quarantined in
  `results/raw/inconclusive/`; the cited unsafe-control result is the deterministic
  loopback differential.

## 7. Optional Docker build (not claimed hermetic)

```bash
docker build -t sage-artifact -f Dockerfile .
docker run --rm sage-artifact    # runs the core verification gate
```
