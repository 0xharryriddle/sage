# SAGE source-artifact checks. Historical experiment and table targets are
# intentionally omitted: this checkout contains no retained campaign outputs.
.PHONY: build test fmt lint verify clean

build:
	cargo build --workspace --locked

test:
	cargo test --workspace --no-fail-fast

fmt:
	cargo fmt --all

lint:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings

verify:
	cargo run -p sage-experiments --bin verify

clean:
	cargo clean
