# Hermetic build + verification image for the SAGE artifact (P2-J).
# Runs the core verification gate so reviewers can reproduce without local setup.
FROM rust:1-bookworm

# OpenJDK for the bounded TLA+/TLC model check (formal/).
RUN apt-get update && apt-get install -y --no-install-recommends \
        default-jre-headless python3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /sage
COPY . .

# Pre-fetch deps as a cache layer, then run the core gate on build.
RUN cargo fetch
RUN rustup component add clippy rustfmt

# Default: the core verification gate from REPRODUCE.md.
CMD bash -lc '\
    set -e; \
    cargo fmt --all -- --check; \
    cargo clippy --workspace --lib; \
    cargo test --workspace; \
    cargo run -p sage-experiments --bin verify; \
    echo "SAGE core verification gate: PASS"'
