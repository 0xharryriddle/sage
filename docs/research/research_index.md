# SAGE Research Index

This folder collects implementation-oriented research for the Rust SAGE system. The goal is to convert the paper-level protocol into engineering decisions backed by deployed systems, BFT literature, and Rust ecosystem choices.

## Documents

- `01_deployed_live_upgrades.md` - Ethereum Merge, Polkadot/Substrate, Cosmos SDK, and Tezos Tenderbake upgrade patterns.
- `02_bft_consensus_and_reconfiguration.md` - PBFT, HotStuff, Tendermint, Flexible BFT, SMR reconfiguration, BFT-SMaRt, Sui Lutris, and DAG-BFT.
- `03_rust_engineering_stack.md` - async runtime, networking, storage, crypto, serialization, simulation, and testing recommendations for Rust.
- `04_sage_design_implications.md` - distilled architecture decisions and requirements for SAGE based on the research.

## High-Level Conclusion

SAGE should be implemented as a Rust workspace with pure deterministic protocol crates, replaceable networking/storage/crypto backends, and a deterministic simulation harness. The first engineering target should be a faithful protocol simulator plus reproducibility artifact. The second target should be a local multi-validator runtime using Tokio, libp2p QUIC, canonical binary encoding, redb storage, and real signature verification behind traits.
