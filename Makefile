# SAGE Makefile — build, test, experiments, verification

.PHONY: test lint fmt verify provenance build clean clean-results formal conformance testbed sota-baseline wan-netem
.PHONY: rq1 rq2 rq3 rq4 overhead safety termination sensitivity scaling manifest_tests tables all-lite all

# ===== Quality Gates =====

build:
	cargo build --workspace

test:
	cargo test --workspace

fmt:
	cargo fmt --all

lint:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings

verify:
	cargo run -p sage-experiments --bin verify

# ===== Figure provenance (plotted coords == source CSVs) =====

provenance:
	python3 scripts/check_figure_provenance.py

# ===== Formal model check (bounded TLA+/TLC) =====

formal:
	bash formal/run_tlc.sh

# ===== TLA+ <-> Rust conformance harness (answers R2-Q8) =====
# Dumps the TLC-reachable committed-vector projections, then replays the SAME
# bounded scenario space through the real Rust cutover-gate predicate and
# asserts the model and code agree on every reachable outcome (faithful: 0
# unsafe; broken control: >=1 unsafe, proving the harness has teeth).
conformance:
	bash formal/dump_states.sh
	cargo test -p sage-node --test formal_conformance -- --nocapture

# ===== Multi-process observed-fork testbed =====

testbed:
	cargo build -p sage-node --bin spawn_testbed --bin validator_proc
	bash scripts/m3_partition_experiment.sh

wan-netem:
	bash scripts/netem_wan_testbed.sh

# Empirical O(n^2) message-complexity sweep over the multi-process testbed.
msg-complexity:
	cargo build -p sage-node --bin spawn_testbed --bin validator_proc
	bash scripts/m4_message_complexity.sh

# Byzantine equivocation: one validator mints two blocks at the cutover height.
# A correct quorum-intersecting engine must NOT fork (0 forks expected).
byzantine:
	cargo build -p sage-node --bin spawn_testbed --bin validator_proc
	bash scripts/m5_byzantine_equivocation.sh

# SOTA live-switch (Cox-style) baseline differential: Cox-style ties SAGE on
# cost (RQ1) but forks 10/10 under partition (no n-f gate); SAGE forks 0/10.
sota-baseline:
	cargo build -p sage-node --bin spawn_testbed --bin validator_proc
	bash scripts/m6_sota_baseline_differential.sh

# ===== Experiments =====

rq1:
	mkdir -p results/raw
	cargo run -p sage-experiments --bin run_rq1 -- \
		--config config/rq1.toml \
		--seeds 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29

rq2:
	mkdir -p results/raw
	cargo run -p sage-experiments --bin run_rq2 -- \
		--config config/rq2.toml \
		--seeds 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14

rq3:
	mkdir -p results/raw
	cargo run -p sage-experiments --bin run_rq3 -- \
		--config config/rq3.toml \
		--kappa 1,2,4,8,16

rq4:
	mkdir -p results/raw
	cargo run -p sage-experiments --bin run_rq4 -- \
		--config config/rq4.toml \
		--seeds 0,1,2,3,4,5,6,7,8,9

overhead:
	mkdir -p results/raw results/tables
	cargo run -p sage-experiments --bin run_overhead -- \
		--config config/rq1.toml \
		--txs-per-block 10,25,50,100,250,500 \
		--seeds 0,1,2,3,4

safety:
	mkdir -p results/raw
	cargo run -p sage-experiments --bin run_safety -- \
		--config config/safety.toml \
		--trials 100

termination:
	mkdir -p results/raw
	cargo run -p sage-experiments --bin run_termination -- \
		--config config/rq2.toml \
		--durations 2,4,6,8,12,16 \
		--trials 10

sensitivity:
	mkdir -p results/raw
	cargo run -p sage-experiments --bin run_sensitivity -- \
		--config config/rq1.toml \
		--delays 40000,80000,160000,320000 \
		--trials 10

scaling:
	mkdir -p results/raw
	cargo run -p sage-experiments --bin run_scaling -- \
		--config config/rq1.toml \
		--validators 4,7,10,13,16,20,25,31,50,100 \
		--trials 20

manifest_tests:
	mkdir -p results/raw
	cargo run -p sage-experiments --bin run_manifest_tests -- \
		--out-dir results/raw

# CutCert timing benchmark (real Ed25519 gen/verify at n=31,64,100,200).
# Release build + real-crypto: debug-mode Ed25519 is too slow for 1000 iters.
cert-timing:
	mkdir -p results/raw
	cargo run --release -p sage-experiments --features real-crypto --bin run_cert_timing -- \
		--validators 31,64,100,200 --iters 1000

# Rollback admissibility: exercises the real RollbackManager fail-closed guard
# across metadata-independent / uncaptured / captured / tampered workload classes.
rollback-admissibility:
	mkdir -p results/raw
	cargo run -p sage-experiments --bin run_rollback_admissibility -- --trials 20

# Local-scale multi-process tier (n up to 31) under software WAN-like profiles.
# LOCAL real-process evidence, not independent-host; single-box liveness valve
# may cut off the largest n (recorded honestly, never a safety failure).
scale-local:
	mkdir -p results/raw
	cargo build -p sage-node --bin spawn_testbed --bin validator_proc
	bash scripts/m7_scale_local.sh 3 "10 16 22 31" 8 45

tables:
	mkdir -p results/tables results/figures
	cargo run -p sage-experiments --bin generate_tables -- \
		--raw-dir results/raw \
		--out-dir results/tables
	cargo run -p sage-experiments --bin generate_figures -- \
		--raw-dir results/raw \
		--out-dir results/figures \
		--allow-missing

# ===== Aggregates =====

all-lite: test verify

all: test verify rq1 rq2 rq3 rq4 safety termination sensitivity scaling manifest_tests tables

# ===== Clean =====

clean:
	cargo clean

clean-results:
	rm -rf results/raw results/tables results/figures results/metadata
