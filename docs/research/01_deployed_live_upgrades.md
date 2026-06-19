# Deployed Live Upgrade and Consensus Transition Research

This document summarizes deployed blockchain upgrade mechanisms relevant to SAGE, with emphasis on what should influence the Rust implementation.

## 1. Ethereum Merge

Sources:

- Ethereum Foundation, Mainnet Merge Announcement: https://blog.ethereum.org/2022/08/24/mainnet-merge-announcement
- Ethereum Paris execution upgrade spec: https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/paris.md
- Ethereum Bellatrix consensus specs: https://github.com/ethereum/consensus-specs/tree/dev/specs/bellatrix
- Ethereum Engine API: https://github.com/ethereum/execution-apis/tree/main/src/engine

### Technical Pattern

Ethereum transitioned from Proof-of-Work execution-chain consensus to Proof-of-Stake Beacon Chain consensus through a staged process:

- Bellatrix activated first on the Beacon Chain at a fixed epoch.
- Paris activated later on the execution chain when Terminal Total Difficulty was reached.
- Consensus-layer and execution-layer clients communicated through the Engine API.
- Operators had to run both client layers during transition.
- Completion was not merely local activation; it depended on post-transition block finalization.

### SAGE-Relevant Lessons

- Separate preparation, activation, finalization, and deprecation phases.
- Prefer objective activation predicates over wall-clock coordination.
- Treat finality as the operational completion criterion.
- Keep old and new components live during a compatibility window.
- Define the cross-component API explicitly. For SAGE, this maps to the shadow engine interface, manifest handoff, and post-cutover target initialization.

## 2. Polkadot and Substrate Forkless Runtime Upgrades

Sources:

- Polkadot consensus documentation: https://docs.polkadot.com/reference/polkadot-hub/consensus-and-security/pos-consensus/
- Polkadot Wiki runtime upgrades: https://wiki.polkadot.com/learn/learn-runtime-upgrades/

### Technical Pattern

Polkadot separates block production from finality:

- BABE handles block production.
- GRANDPA finalizes chains of blocks.
- Runtime logic is stored on-chain as Wasm.
- Governance can replace runtime code through a state transition.
- Nodes execute the new runtime after the finalized upgrade block without a conventional hard fork.

### SAGE-Relevant Lessons

- Consensus-critical or migration-critical logic should be versioned and bound to finalized state.
- SAGE should represent schedules, engine generations, and migration epochs in platform state, not only in local config files.
- The client-facing interface should remain stable even when internal consensus changes.
- A clear separation between block production, finality, and runtime state helps isolate migration logic.

## 3. Cosmos SDK Upgrade Module

Sources:

- Cosmos SDK upgrade module: https://docs.cosmos.network/sdk/latest/upgrade/upgrade
- Go package docs for `x/upgrade`: https://pkg.go.dev/github.com/cosmos/cosmos-sdk/x/upgrade
- Cosmovisor: https://github.com/cosmos/cosmos-sdk/blob/main/tools/cosmovisor/README.md

### Technical Pattern

Cosmos uses governance-scheduled upgrade plans:

- A plan is scheduled at a specific height.
- Validators install a binary with the corresponding upgrade handler.
- At the upgrade height, old binaries halt or panic if the correct handler is absent.
- Cosmovisor can automate binary switching.
- This model accepts planned downtime or near-downtime to prevent incompatible state machines from continuing.

### SAGE-Relevant Lessons

- Even if SAGE targets zero-halt migration, it should include a safe halt/fail-closed fallback for impossible or invalid migration states.
- Upgrade plans should include name, height, epoch, expected tool hash, and expected source/target engine generations.
- Missing migration handlers or invalid manifests should fail closed.
- SAGE's stop-the-world baseline should not be dismissed; it is the safety fallback.

## 4. Tezos Tenderbake and On-Chain Amendment

Sources:

- Tezos amendment history: https://docs.tezos.com/architecture/governance/amendment-history
- Octez Tenderbake consensus docs: https://octez.tezos.com/docs/active/consensus.html
- Nomadic Labs Tenderbake activation: https://research-development.nomadic-labs.com/activating-tenderbake.html

### Technical Pattern

Tezos replaced its consensus protocol through protocol-native amendment:

- Protocol amendments proceed through governance phases.
- The Ithaca amendment activated Tenderbake.
- Tenderbake provides deterministic BFT-style finality.
- Consensus is protocol-versioned and amendable.

### SAGE-Relevant Lessons

- Consensus replacement should be treated as an explicitly versioned protocol change.
- Governance legitimacy and activation semantics matter as much as implementation mechanics.
- Migration artifacts should be auditable after activation.
- SAGE manifests should support operator evidence bundles.

## 5. Cross-System Design Patterns

SAGE should adopt these patterns:

1. Multi-phase transitions: prepare, dual-run, certify, cut over, provisional window, seal.
2. Objective activation predicates: finalized height, migration epoch, readiness certificate, and manifest verification.
3. Explicit compatibility windows: both engines exist, but only one finalizes.
4. Finality-aware completion: migration is not complete until the target is sealed or reaches a defined finality boundary.
5. Fail-closed behavior: missing handler, invalid root, wrong epoch, or bad manifest must refuse cutover.
6. Auditable artifacts: every migration should emit a manifest, certificates, configuration hash, and logs sufficient for external verification.
