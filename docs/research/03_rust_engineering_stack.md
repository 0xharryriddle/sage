# Rust Engineering Stack Research

This document records recommended Rust technologies and engineering practices for SAGE.

## 1. Async Runtime

Sources:

- Tokio docs: https://docs.rs/tokio
- Tokio runtime docs: https://docs.rs/tokio/latest/tokio/runtime/struct.Runtime.html
- Tokio site: https://tokio.rs/

### Recommendation

Use Tokio as the default async runtime for node and local-testbed code.

### Rationale

- It is the dominant Rust runtime for networked systems.
- It provides async I/O, timers, task scheduling, synchronization, and blocking pools.
- It integrates well with libp2p and common observability tooling.

### SAGE Rule

Keep core consensus deterministic and mostly synchronous. Tokio should drive I/O and scheduling around consensus, not be embedded into pure safety logic.

## 2. Networking

Sources:

- `libp2p-quic`: https://docs.rs/libp2p-quic
- libp2p QUIC docs: https://libp2p.io/docs/quic/
- libp2p Kademlia docs: https://docs.rs/libp2p/latest/libp2p/kad/index.html
- libp2p Gossipsub crate: https://docs.rs/crate/libp2p-gossipsub/latest

### Recommendation

Use `rust-libp2p` with QUIC as the primary transport for the local multi-validator runtime.

### Rationale

- QUIC combines transport, encryption, and multiplexing.
- It avoids TCP head-of-line blocking between independent streams.
- It has one-RTT setup and maps well to many protocol streams.
- libp2p provides peer identity, gossip, discovery, request-response, identify, and ping behaviors.

### SAGE Usage

- QUIC transport for node connections.
- Gossipsub for blocks, attestations, and votes where gossip is appropriate.
- Request-response for block sync, state chunks, manifests, and proofs.
- Kademlia or static peer lists for discovery depending on deployment mode.

## 3. Storage

Sources:

- redb: https://github.com/cberner/redb
- parity-db: https://crates.io/crates/parity-db
- RocksDB Rust wrapper: https://docs.rs/rocksdb

### Recommendation

Create a `sage-store` interface immediately. Start with `redb`; evaluate RocksDB or parity-db later.

### Rationale

`redb` is pure Rust, embedded, ACID, copy-on-write, and simple to operate. RocksDB is mature and tunable but brings native dependencies and compaction complexity. `parity-db` is blockchain-oriented and worth evaluating once SAGE's state shape is known.

### SAGE Storage Interfaces

- Blocks and headers.
- Finalized roots.
- Certificates and manifests.
- Validator sets and config epochs.
- State checkpoints.
- Provisional suffix for rollback.
- Safety-critical consensus metadata.

## 4. Hashing

Sources:

- RustCrypto SHA-2: https://crates.io/crates/sha2
- RustCrypto hashes: https://github.com/rustcrypto/hashes
- BLAKE3: https://crates.io/crates/blake3

### Recommendation

Use SHA-256 for conservative protocol compatibility and BLAKE3 for internal high-performance content hashing where standardization is not required.

### Rules

- Domain-separate every hash.
- Hash canonical bytes only.
- Include version bytes in consensus-critical encodings.
- Never hash ambiguous concatenations.

## 5. Signatures and Crypto

Sources:

- ed25519-dalek: https://docs.rs/ed25519-dalek/
- k256: https://crates.io/crates/k256
- blst: https://crates.io/crates/blst
- blst upstream: https://github.com/supranational/blst
- bls12_381: https://docs.rs/bls-12-381

### Recommendation

Use traits for all cryptographic operations. Start with simulated certificates for experiments, then add real signatures.

Suggested mapping:

- `ed25519-dalek` for validator and peer signatures.
- `k256` if Ethereum/secp256k1 compatibility is needed.
- `blst` for BLS12-381 aggregate/threshold-style signatures.
- Pure Rust BLS or arkworks crates only for research/prototyping unless performance is acceptable.

### SAGE Rule

The simulator may model threshold signatures as quorum sets, but production-adjacent code must verify actual signatures over canonical bytes.

## 6. Serialization

### Recommendation

Use `serde` for config/result files, but define canonical binary encodings for consensus messages.

Potential crates:

- `postcard` for compact serde-compatible binary encoding.
- `bincode` with explicit fixed options.
- custom codec for critical objects if needed.

### Rules

- Hash and sign only canonical bytes.
- Include schema version and domain labels.
- Avoid JSON for consensus messages.
- Fuzz decoders.

## 7. Simulation and Testing

Sources:

- proptest: https://docs.rs/proptest
- loom: https://docs.rs/loom/latest/loom/
- loom GitHub: https://github.com/tokio-rs/loom
- Turmoil deterministic async testing article: https://s2.dev/blog/dst
- madsim: https://github.com/madsim-rs/madsim

### Recommendation

Make deterministic simulation a first-class subsystem.

Use:

- Unit tests for pure state-transition functions.
- `proptest` for generated message/block/fork sequences.
- `loom` for concurrency-sensitive code.
- `turmoil` for deterministic multi-node async network tests.
- `cargo-fuzz` for parsers, block validation, storage recovery, and signature/message decoders.

### SAGE Testing Targets

- Conflicting commits are impossible under SAGE assumptions.
- Hard fork and blind cutover can produce conflicts under partition.
- Rollback cannot revert absolute-final blocks.
- Manifest replay is rejected.
- Shadow engine cannot finalize.
- Crashed validators cannot double vote after restart.

## 8. Initial Dependency Direction

Approximate starting dependencies:

```toml
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time", "net"] }
tracing = "0.1"
tracing-subscriber = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
thiserror = "1"
rand = "0.8"
rand_chacha = "0.3"
sha2 = "0.10"
blake3 = "1"
ed25519-dalek = { version = "2", features = ["rand_core", "serde", "zeroize"] }
redb = "2"
proptest = "1"
clap = { version = "4", features = ["derive"] }
csv = "1"
```

Add libp2p, BLS, RocksDB, and deterministic async simulation after the pure simulator and core protocol are stable.

## 9. Architecture Rules

- Consensus crates should not depend on libp2p.
- Storage should be behind traits.
- Crypto should be behind traits.
- Config must be immutable during a run except through ordered governance/config commands.
- All RNG use must be seed-controlled in simulations.
- Result schemas must be stable and versioned.
