# SAGE — Shadow-Anchored Graceful Evolution

**SAGE** is the project and protocol-design name: **Shadow-Anchored Graceful
Evolution**. It studies certified migration between deterministic-finality
consensus engines in a permissioned blockchain with one fixed, identical
validator committee and monolithic state.

The canonical paper is **“SAGE: A Certified Cross-Engine Boundary for
Fixed-Committee Consensus Migration in Permissioned Blockchains,”** at
[`docs/paper/SAGE_ThaiCong_IEEE/main.tex`](docs/paper/SAGE_ThaiCong_IEEE/main.tex).
It uses Elsevier's `elsarticle` class for *Blockchain: Research and
Applications*; `_IEEE` in the directory name is historical. The older
[`docs/paper.tex`](docs/paper.tex) is a retained legacy draft.

SAGE is a conditional protocol design and research artifact, not a
production-qualified migration implementation. Source finalization continues
during shadow preparation, followed by a deliberate **certification pause**
before target activation. “Live migration” in the paper does not mean
uninterrupted consensus.

## Protocol design

The paper's locked-readiness profile separates migration obligations that a
simple engine or quorum swap would conflate:

1. **Single-finalizer shadow execution.** The source engine remains the only
   finalizer while the target re-executes source-finalized blocks without
   proposing or finalizing.
2. **Durable readiness lock.** After a stability window, a correct validator
   locks one eligible boundary and durably stops old-generation source
   authorization above it before releasing its readiness share.
3. **Certified boundary.** An `n-f` `CutCert`, under the admitted source policy
   `q1 > 2f`, excludes further source finalization. Target authority is still
   disabled at this point.
4. **Manifest and retirement evidence.** An `n-f` `ManifestCert`, a durable
   local source fence, and a policy-bound `FenceCert` gate target activation.
   In the locked design, `FenceCert` records manifest-bound retirement; it is
   not an independent source-exclusion necessity.
5. **Two-tier finality.** Target blocks are provisional until a certified
   seal. A certified abort before the deadline returns to the boundary anchor
   under a fresh source generation; reaching the deadline alone does not seal.
6. **Fail-closed progress.** Incompatible candidates or insufficient eligible
   signers may leave migration pending. Automatic close-and-retry is outside
   the current design.

For arbitrary-subset boundary certificates, correct-signer intersection and
availability require

```text
(n + f) / 2 < q <= n - f
```

SAGE chooses `q = n-f`, the largest available threshold, not the only safe
one. This applies established Byzantine-quorum intersection to migration
authority; the paper does not claim new quorum arithmetic or that `2f+1` is
unsafe in every BFT protocol.

## What this repository contains

| Surface | Included scope | What it does **not** establish |
|---|---|---|
| Rust workspace | Data model, manifest/consensus/controller crates, deterministic simulator, experiment tools, in-memory storage/network abstractions, and a partial local process runtime | Complete production handoff or deployment readiness |
| Formal models | Bounded TLA+/TLC design abstractions with falsifying controls for boundary decisions, manifest agreement, fence ordering, rollback, readiness locking, and selected committee layouts | General theorem, model-to-code refinement, or runtime authority |
| Simulator | Deterministic event-time controls, parameter sensitivity, and artifact generation | Client-visible downtime or full network-path performance |
| Multi-process testbed | Constructed loopback TCP controls for a partial PoA-to-HotStuff path | Complete committee-wide manifest/fence/abort/seal execution, authenticated placement, or physical geo-WAN evidence |
| Paper rebuild | Hash-checks retained inputs and regenerates presentation tables, figures, and PDF products | Historical experiment rerun, missing trial recovery, submission approval, or runtime authorization |

The implemented process path is a `CutCert`-gated subset with a crash-only
source profile. Committee-wide manifest gossip, fence-certificate transport,
terminal-certificate dissemination, certified abort and seal, and persistent
Rule 1 enforcement through the complete handoff remain open system work.

## Quick start

```bash
cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --no-fail-fast
cargo run -p sage-experiments --bin verify
```

`verify` loads the checked-in configurations itself; it has no `--config`
argument. Its checks cover configuration loading, deterministic simulator
smokes, selected invariants, schema constants, and headers of raw CSVs that
exist in the checkout. A passing verifier is not validation of retained CSV
rows, the full network protocol, or paper submission readiness.

Experiment commands create **new observations**. For example:

```bash
cargo run -p sage-experiments --bin run_rq1 -- \
  --config config/rq1.toml --seeds 0,1,2
```

This small run does not reproduce the retained paper campaign. See
[`docs/EXPERIMENT_FAIRNESS.md`](docs/EXPERIMENT_FAIRNESS.md) before interpreting
strategy contrasts or comparing simulator outputs.

## Paper rebuild

Three paper-specific inputs whose historical top-level paths contain different
legacy bytes live under `results/raw/paper/SAGE_ThaiCong_IEEE/`. Existing legacy
CSVs remain unchanged.

```bash
python3 -B docs/paper/SAGE_ThaiCong_IEEE/rebuild.py --check-data
python3 -B docs/paper/SAGE_ThaiCong_IEEE/rebuild.py \
  --verify-rebuild --work-root /tmp/sage-paper-build
```

The first command checks eight SHA-256-pinned CSV inputs and two pinned notes.
The second performs two isolated builds, runs the staged manuscript gates, and
compares 22 presentation products byte-for-byte. Neither command reruns
historical campaigns or changes protocol authority.

## Evaluation boundaries

- The retained RQ1 cost comparison is a partial-path simulator study with an
  inadmissible source policy and unequal target quorums. Its finalization gaps
  are simulator event time, not complete certification cost or client downtime.
- The internal `CoxStyle` arm is not an implementation or benchmark of Cox's
  checkpoint/recovery protocol. No external switching implementation has been
  executed in the shared harness.
- Process controls separate unsafe-threshold conflicts from gate authorization
  on constructed schedules. They do not execute the complete source-retirement
  adversary or prove `n-f` is the unique safe threshold.
- Configured-endpoint records do not independently authenticate machine
  identity, placement, impairment, or physical isolation.
- Bounded model checks support only their finite abstractions. Broken controls
  are expected to produce named counterexamples.

See [`docs/ARTIFACT_EVALUATION.md`](docs/ARTIFACT_EVALUATION.md) for the
claim-to-command map and [`docs/RESPONSE_TO_REVIEWS.md`](docs/RESPONSE_TO_REVIEWS.md)
for the historical response plus current-scope warnings.

## Workspace

```text
crates/
  sage-core/         deterministic protocol data model
  sage-manifest/     manifests, certificates, and signature modeling
  sage-consensus/    consensus engines and quorum primitives
  sage-controller/   migration state machine and invariants
  sage-sim/          deterministic event-driven simulator
  sage-stats/        statistical utilities
  sage-experiments/  experiment and artifact-generation CLIs
  sage-store/        storage traits and in-memory backend
  sage-network/      transport abstractions
  sage-node/         partial local multi-process runtime
formal/              bounded TLA+/TLC design models
config/              simulator and experiment configurations
docs/                manuscript, evaluation guides, and research notes
results/             retained inputs and generated/experimental artifacts
scripts/             experiment, verification, and editorial tools
```

Important entry points:

- [`docs/README.md`](docs/README.md) — documentation and manuscript index
- [`docs/paper/SAGE_ThaiCong_IEEE/main.tex`](docs/paper/SAGE_ThaiCong_IEEE/main.tex) — canonical manuscript
- [`docs/ARTIFACT_EVALUATION.md`](docs/ARTIFACT_EVALUATION.md) — evidence tiers and commands
- [`docs/EXPERIMENT_FAIRNESS.md`](docs/EXPERIMENT_FAIRNESS.md) — comparison conditions and confounders
- [`formal/README.md`](formal/README.md) — model inventory, direct TLC commands, and limits
- [`Makefile`](Makefile) — build, test, simulator, and testbed targets

## Known gaps

- No complete networked manifest/fence/activation/abort/seal campaign.
- No admitted, quorum-matched full-handoff RQ1 with client-completed throughput
  and interruption measurements.
- No common-harness execution of an artifact-backed contemporary switching
  system or Fabric-style maintenance transition.
- No implementation refinement proof from the bounded models to Rust.
- No safe close-and-retry protocol for incompatible candidate locks.
- No joint committee/engine migration, sharded state, probabilistic-finality
  source, or automatic compensation for discarded provisional effects.
- Candidate Retry implementation authority remains separately exact-byte
  governed and is not granted by this repository's ordinary checks.

## License

MIT OR Apache-2.0
