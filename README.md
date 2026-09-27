# SAGE — Shadow-Anchored Graceful Evolution

SAGE is a research implementation for migrating permissioned blockchains between deterministic-finality consensus engines under a fixed committee and monolithic state. The workspace contains protocol and consensus crates, a deterministic simulator, experiment generators, and a partial local process testbed. It is not a production-qualified migration implementation.

The documented design intent keeps the source engine as sole finalizer during shadow validation and inserts a certification pause before target activation. This checkout contains only a partial protocol path; it does not demonstrate that complete handoff or production service guarantees.

## Scope

- The simulator reports deterministic model behavior and event-time metrics; these are not client-visible throughput or downtime measurements.
- Local process tests use loopback TCP and exercise only implemented paths. They do not run the complete networked manifest/fence/abort/seal handoff.
- TLC checks are finite model checks for their configured systems. They are not an unbounded proof or complete Rust trace-refinement proof.
- Experiment binaries may write under ignored `results/`; the process-testbed scripts also write per-run CSV files under `/tmp`. Neither location in this checkout contains retained historical evidence.

## Quick start

Requirements: Rust stable with Cargo, Java for TLC, and a POSIX shell for Make targets. `Cargo.toml` declares workspace edition 2021; no `rust-toolchain.toml` pin is included.

```bash
cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --no-fail-fast
cargo run -p sage-experiments --bin verify
```

Use `--no-fail-fast` so later workspace suites run after a failure; report the actual test summary and do not treat a nonzero exit as green.

Captured public-clean-checkout validation: the `cargo test --workspace --no-fail-fast` output reports 85 passed, 0 failed, and 2 ignored across 11 targets with tests. The three registered RED selectors for gated source work are absent from this run; this is not full-workspace validation. `make verify` reported 16 passed, 0 failed; `cargo fmt --all -- --check` passed. `make lint` remains blocked by Clippy's `clippy::for_kv_map` warning at `crates/sage-sim/src/simulation.rs:179`; this source-only scope does not modify that code. No retained campaign outputs are available for a fresh historical-data audit.

`verify` loads checked-in simulator configurations, runs deterministic scenarios, checks the initial controller snapshot in its SAGE scenario, validates experiment schema constants, and checks headers of selected CSVs only when those files exist under `results/raw/`. It does not validate CSV rows, retained evidence, model projections, or quorum-policy artifacts. This checkout includes no historical result CSVs.

## Bounded formal models

`formal/README.md`, the TLA+ source comments, and the inherited `formal/run_tlc.sh` contain historical paper/review/testbed claims that this checkout does not authenticate. Treat those claims as unverified context, not as evidence of empirical results or end-to-end protocol behavior. The runner also has an EXIT cleanup trap for `formal/states/`; preserve any valuable local state before invoking it.

## Experiments

Cargo experiment binaries and the M3–M6 shell scripts remain available for fresh runs; the Makefile defines no campaign targets. Direct script runs may write per-run CSVs under `/tmp` and aggregate outputs under ignored `results/raw/`. These new outputs do not reproduce the omitted historical campaigns.

## License

MIT OR Apache-2.0
