# SAGE — Artifact Evaluation Guide

This guide maps the **clone-visible** SAGE artifact to commands that exist on
`main`. It separates deterministic simulation, bounded formal models,
constructed loopback process controls, retained presentation inputs, and the
paper rebuild. None of these tiers alone establishes a complete networked
migration, production readiness, authenticated deployment geography, or paper
submission readiness.

The canonical manuscript is
`docs/paper/SAGE_ThaiCong_IEEE/main.tex`. Its central claim is a conditional
certified authority-boundary design for one fixed, identical committee—not an
executed end-to-end system.

## 1. Current claim boundary

- **Design argument:** source finalization continues during shadow preparation;
  validators then enter a deliberate certification pause. Under durable Rule 1
  and an admitted `q1 > 2f` source policy, an `n-f` `CutCert` excludes further
  old-generation source finalization. `FenceCert` supplies manifest- and
  policy-bound retirement evidence in that locked profile.
- **Formal evidence:** bounded TLA+/TLC design abstractions with deliberate
  broken controls. These are not a general proof or model-to-code refinement.
- **Simulator evidence:** deterministic partial-path behavior and event-time
  metrics, not client-visible downtime or complete certificate transport.
- **Process evidence:** constructed loopback controls for a partial
  PoA-to-HotStuff path. The full manifest/fence/abort/seal path is not executed.
- **Presentation evidence:** a hash-checked manuscript rebuild from retained
  inputs. It does not rerun experiments or recover missing trial records.

RQ1 remains an inadmissible, quorum-confounded partial-path comparison. The
internal `CoxStyle` arm is not Cox. Configured endpoint labels do not attest
machine identity, placement, impairment, or physical isolation.

## 2. Local workspace checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --no-fail-fast
cargo run -p sage-experiments --bin verify
```

`verify` loads the checked-in configurations itself. It covers configuration
loading, deterministic smokes, selected invariants, schema constants, and raw
CSV headers that exist in the checkout. It does not validate retained CSV rows,
the complete network protocol, or paper readiness.

## 3. Canonical paper rebuild

```bash
python3 -B docs/paper/SAGE_ThaiCong_IEEE/rebuild.py --check-data
python3 -B docs/paper/SAGE_ThaiCong_IEEE/rebuild.py \
  --verify-rebuild --work-root /tmp/sage-paper-build
```

The first command validates eight SHA-256-pinned CSVs and two pinned notes. The
second performs two isolated builds, runs the staged manuscript gates, and
compares 22 presentation products byte-for-byte. Three paper-specific inputs
reside under `results/raw/paper/SAGE_ThaiCong_IEEE/`; distinct legacy CSVs at
their historical top-level paths remain unchanged.

Scope: presentation regeneration only. The build does not rerun campaigns,
authenticate historical environments, grant runtime authority, or establish
submission readiness.

## 4. Bounded formal models

`formal/README.md` lists each model, faithful configuration, falsifying control,
and direct TLC command. Use metadata outside `formal/states/`.

```bash
make formal
bash formal/check_readiness_lock.sh
```

The legacy aggregate runner covers its historical model set and intentionally
expects broken controls to produce named counterexamples. **It also removes
`formal/states/` on exit.** Run `make formal` only in a disposable checkout with
no valuable local TLC state. In a populated checkout, use the isolated
`-metadir` direct TLC commands documented in `formal/README.md`. The separate
readiness-lock runner does not make the legacy runner cover the newer models.

## 5. Fresh simulator outputs

These targets create new observations under the current code and config; they
do not reproduce the retained paper campaigns merely by reusing filenames or
seed labels:

```bash
make rq1
make rq2
make rq3
make rq4
make safety
make termination
make sensitivity
make scaling
make manifest_tests
make tables
```

Interpretation limits:

- RQ1 reports simulator finalization gaps, not end-to-end certification pause
  or client downtime.
- RQ2/safety `disjoint_quorum_windows` is exposure, not an observed conflict.
- RQ4 computes an abort-window/admissibility proxy; it does not execute
  `AbortCert` or certified rollback.
- Generated tables and figures summarize supplied rows; they do not authenticate
  those rows as admitted evidence.

## 6. Constructed process controls

```bash
make testbed
make msg-complexity
make byzantine
make sota-baseline
```

These invoke the four tracked shell harnesses under `scripts/`. They exercise
real OS processes over loopback TCP but remain constructed, single-host,
partial-path controls. Historical Cox-style labels identify internal controls,
not an authentic Cox implementation or external benchmark.

## 7. Retained public inputs

The checkout includes the exact inputs needed for the canonical presentation
build and selected legacy experiment CSV/metadata. Presence in Git does not
authenticate provenance or promote an old result to a current execution receipt.

- Preserve `results/raw/paper/SAGE_ThaiCong_IEEE/` as immutable paper inputs.
- Do not overwrite hash-pinned inputs with fresh experiment output.
- Keep aggregate-only results labeled aggregate-only; do not manufacture trial
  rows or confidence intervals from unavailable samples.
- Do not infer physical deployment facts from endpoint labels.

## 8. Evidence matrix

| Claim surface | Available evidence | Principal limit |
|---|---|---|
| Boundary/decision design | Paper arguments + bounded models | Conditional premises; no refinement proof |
| Manifest agreement/fence ordering | Separate bounded models with broken controls | Not network dissemination |
| No-fault simulator behavior | `verify` and experiment binaries | Partial path; simulator event time |
| Constructed partition controls | Loopback process harnesses | Single host; selected schedules |
| Certificate CPU/size presentation | Pinned retained aggregate CSV | No distributed quorum latency; samples unavailable |
| Paper tables/figures/PDF | Two-build presentation comparison | No experiment rerun or submission GO |

## 9. Missing system evidence

- Complete committee-wide `ManifestCert`, `FenceCert`, `AbortCert`, and
  `SealCert` transport and execution.
- Persistent Rule 1 enforcement across every relevant crash point.
- An admitted, quorum-matched full-handoff cost campaign with client-completed
  throughput and interruption measurements.
- An artifact-backed external switching comparator on the same harness.
- Authenticated physical geo-WAN placement and impairment records.
- Client receipt reconciliation and compensation for discarded provisional
  effects.

Use `docs/EXPERIMENT_FAIRNESS.md` for comparison confounders,
`docs/RESPONSE_TO_REVIEWS.md` for historical review context, and
`formal/README.md` for bounded-model scope.
