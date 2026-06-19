# SAGE Makefile — build, test, experiments, verification

.PHONY: test lint fmt verify build clean clean-results formal testbed sota-baseline
.PHONY: rq1 rq2 rq3 rq4 safety termination sensitivity scaling manifest_tests tables all-lite all

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

# ===== Formal model check (bounded TLA+/TLC) =====

formal:
	bash formal/run_tlc.sh

# ===== Multi-process observed-fork testbed =====

testbed:
	cargo build -p sage-node --bin spawn_testbed --bin validator_proc
	bash scripts/m3_partition_experiment.sh

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
		--validators 4,7,10,13,16,20 \
		--trials 10

manifest_tests:
	mkdir -p results/raw
	cargo run -p sage-experiments --bin run_manifest_tests -- \
		--out-dir results/raw

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
