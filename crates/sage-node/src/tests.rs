//! Local-runtime and multi-process integration tests for `sage-node`.
//!
//! Extracted verbatim from the `#[cfg(test)] mod tests` block in `lib.rs` for
//! maintainability. Declared as `#[cfg(test)] mod tests;` in `lib.rs`, so this
//! is the same child module (full access to crate-private items via
//! `use super::*`) — no visibility changes, identical test set.
use super::*;
use sage_controller::Schedule;

fn no_fault_schedule() -> Schedule {
    Schedule {
        h_d: Height::new(1),
        h_c: Height::new(3),
        h_r: Height::new(7),
        kappa: 1,
        tau_blocks: 1,
    }
}

#[test]
fn poa_finalizes_blocks() {
    let cfg = NodeConfig::local_testnet(1, 0, no_fault_schedule(), Height::new(3));
    let mut rt = LocalRuntime::new(cfg).unwrap();
    let m = rt.run().unwrap();
    assert!(m.finalized_blocks > 0, "PoA should finalize blocks");
    assert!(!m.safety_violation, "no safety violations");
}

#[cfg(feature = "real-crypto")]
#[test]
fn cutover_attestation_rejects_wrong_domain_and_unknown_sender() {
    let cfg = NodeConfig::local_testnet(4, 1, no_fault_schedule(), Height::new(5));
    let mut node = build_validator(ValidatorId::new(0), &cfg);
    let boundary = Hash32::new([7; 32]);
    let root = Hash32::new([8; 32]);
    let target_engine = node.target.engine_id();
    let message = |from, chain_id, epoch, config_id, tamper_root| {
        let signing_node = build_validator(from, &cfg);
        let signature = signing_node
            .sign_cutover_attestation(cfg.migration.h_c, boundary, root, target_engine)
            .unwrap();
        MessageEnvelope {
            from,
            to: None,
            chain_id,
            epoch,
            config_id,
            message: ConsensusMessage::Sage(SageMessage::CutoverAttestation {
                height: cfg.migration.h_c,
                boundary_block: boundary,
                boundary_root: if tamper_root { Hash32::ZERO } else { root },
                target_engine,
                signature,
            }),
        }
    };

    assert!(node
        .handle_message(message(
            ValidatorId::new(99),
            cfg.chain_id.clone(),
            cfg.epoch,
            cfg.config_id,
            false,
        ))
        .is_err());
    assert!(node
        .handle_message(message(
            ValidatorId::new(1),
            sage_core::ChainId::new("other-chain"),
            cfg.epoch,
            cfg.config_id,
            false,
        ))
        .is_err());
    assert!(node.cutover_attesters.is_empty());

    node.handle_message(message(
        ValidatorId::new(1),
        cfg.chain_id.clone(),
        cfg.epoch,
        cfg.config_id,
        false,
    ))
    .unwrap();
    assert_eq!(
        node.cutover_attesters
            .get(&CutoverKey {
                height: cfg.migration.h_c,
                boundary_block: boundary,
                boundary_root: root,
                target_engine,
            })
            .map(|shares| shares.len()),
        Some(1)
    );

    assert!(node
        .handle_message(message(
            ValidatorId::new(2),
            cfg.chain_id.clone(),
            cfg.epoch,
            cfg.config_id,
            true,
        ))
        .is_err());
    assert_eq!(
        node.cutover_attesters
            .get(&CutoverKey {
                height: cfg.migration.h_c,
                boundary_block: boundary,
                boundary_root: root,
                target_engine,
            })
            .map(|shares| shares.len()),
        Some(1),
        "tampered signed payload must not increase quorum"
    );

    let conflicting_root = Hash32::new([9; 32]);
    let conflicting_signer = build_validator(ValidatorId::new(2), &cfg);
    let conflicting_signature = conflicting_signer
        .sign_cutover_attestation(cfg.migration.h_c, boundary, conflicting_root, target_engine)
        .unwrap();
    node.handle_message(MessageEnvelope {
        from: ValidatorId::new(2),
        to: None,
        chain_id: cfg.chain_id.clone(),
        epoch: cfg.epoch,
        config_id: cfg.config_id,
        message: ConsensusMessage::Sage(SageMessage::CutoverAttestation {
            height: cfg.migration.h_c,
            boundary_block: boundary,
            boundary_root: conflicting_root,
            target_engine,
            signature: conflicting_signature,
        }),
    })
    .unwrap();

    let canonical_key = CutoverKey {
        height: cfg.migration.h_c,
        boundary_block: boundary,
        boundary_root: root,
        target_engine,
    };
    let conflicting_key = CutoverKey {
        boundary_root: conflicting_root,
        ..canonical_key
    };
    assert_eq!(
        node.cutover_attesters
            .get(&canonical_key)
            .map(|shares| shares.len()),
        Some(1),
        "a valid signature for another root must not increase this quorum"
    );
    assert_eq!(
        node.cutover_attesters
            .get(&conflicting_key)
            .map(|shares| shares.len()),
        Some(1),
        "conflicting valid payload must remain in its own evidence bucket"
    );
}

/// Regression: the wall-clock runtime must cross the PoA->HotStuff cutover
/// for n>=5. Before the view-keyed proposer fix it stalled at h_c-1 because
/// the runtime selected the proposer by height while the engine validated by
/// view; once the view ran away after the first failed round nobody could
/// propose. n=6 is the `run_node` default that surfaced the stall.
#[test]
fn six_validator_sage_crosses_cutover() {
    let mut cfg = NodeConfig::local_testnet(6, 1, no_fault_schedule(), Height::new(5));
    cfg.max_runtime_secs = 5;
    let mut rt = LocalRuntime::new(cfg).unwrap();
    let m = rt.run().unwrap();
    assert!(
        m.max_finalized_height >= 4,
        "n=6 must finalize past the cutover height (h_c=3), got {}",
        m.max_finalized_height
    );
    assert!(
        m.migration_success,
        "n=6 migration should complete through target finality"
    );
    assert!(!m.safety_violation, "no safety violations");
}

/// Regression: the wall-clock runtime must also cross the cutover at higher
/// validator counts. Before the monotonic per-view timer (research Bug C)
/// the pacemaker emitted a timeout every poll; 2f+1 timeouts formed a TC
/// that cleared pending_votes before the lone leader's votes reached quorum,
/// so n>=8 raced the view to 100+ and never finalized the first HotStuff
/// block. n=12 is well past that boundary.
#[test]
fn twelve_validator_sage_crosses_cutover() {
    let mut cfg = NodeConfig::local_testnet(12, 1, no_fault_schedule(), Height::new(5));
    cfg.max_runtime_secs = 8;
    let mut rt = LocalRuntime::new(cfg).unwrap();
    let m = rt.run().unwrap();
    assert!(
        m.max_finalized_height >= 4,
        "n=12 must finalize past the cutover height (h_c=3), got {}",
        m.max_finalized_height
    );
    assert!(
        m.migration_success,
        "n=12 migration should complete through target finality"
    );
    assert!(!m.safety_violation, "no safety violations");
}

#[test]
fn four_validator_poa() {
    let cfg = NodeConfig::local_testnet(4, 1, no_fault_schedule(), Height::new(4));
    let mut rt = LocalRuntime::new(cfg).unwrap();
    let m = rt.run().unwrap();
    assert!(m.finalized_blocks > 0, "should finalize blocks");
}

#[test]
fn process_result_reports_replay_context_root_for_reversible_window() {
    use sage_core::{BlockHeader, EngineGeneration, FinalityTier, Transaction};

    let cfg = NodeConfig::local_testnet(1, 0, no_fault_schedule(), Height::new(5));
    let id = ValidatorId::new(0);
    let transport = InMemoryTransport::new([id]);
    let mut pv = ProcessValidator::new(id, cfg.clone(), transport);
    pv.node.cutover_height = Some(cfg.migration.h_c);
    let engine_id = EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1));

    pv.node.committed.push(FinalizedBlock {
        block: Block {
            header: BlockHeader {
                chain_id: cfg.chain_id.clone(),
                epoch: cfg.epoch,
                config_id: cfg.config_id,
                height: cfg.migration.h_c,
                parent_hash: Hash32::ZERO,
                state_root: Hash32::new([7; 32]),
                engine_id,
                finality_tier: FinalityTier::Absolute,
                manifest_hash: None,
            },
            txs: vec![Transaction {
                from: 0,
                to: 1,
                amount: 1,
                nonce: 0,
            }],
        },
        certificate_hash: Hash32::new([8; 32]),
    });

    let result = pv.build_result(0.0);
    assert!(
        result.replay_context_root.is_some(),
        "reversible-window finalized blocks must surface a replay-context root"
    );
    assert!(
        result.manifest_payload_hash.is_some(),
        "process results must surface the manifest payload hash that binds replay context"
    );
}

/// M1: several independent `ProcessValidator`s, each with NO shared node
/// memory, drive the SAME consensus over a shared message bus and must cross
/// the PoA->HotStuff cutover and AGREE on every committed block hash. This
/// exercises the exact multi-process driver path (every message, including
/// the leader's own proposal/vote, round-trips through the transport) using
/// the fast in-memory bus instead of real sockets. The TCP path is the same
/// driver over `TcpTransport`.
///
/// Marked `#[ignore]`: the 6 validator loops are real OS threads driven by a
/// wall-clock pacemaker, so under full `cargo test --workspace` parallel load
/// they get CPU-starved and a view timer can miss its deadline (intermittent
/// ~1/8). It passes reliably in isolation
/// (`cargo test -p sage-node --lib multi_process -- --ignored`). The
/// authoritative, truly-parallel artifact is the multi-process `spawn_testbed`
/// (`make testbed`), which runs separate processes rather than starved threads.
#[test]
#[ignore = "timing-dependent; threads starve under workspace-parallel load. \
                Run on demand: `cargo test -p sage-node --lib multi_process -- --ignored`. \
                Authoritative artifact: `make testbed`."]
fn multi_process_validators_agree_across_cutover() {
    use sage_network::SharedMemoryTransport;
    use std::collections::BTreeMap;

    let n = 6u32;
    let ids: Vec<ValidatorId> = (0..n).map(ValidatorId::new).collect();
    let transports = SharedMemoryTransport::mesh(&ids);

    let handles: Vec<_> = transports
        .into_iter()
        .enumerate()
        .map(|(i, transport)| {
            let mut cfg = NodeConfig::local_testnet(n, 1, no_fault_schedule(), Height::new(5));
            cfg.max_runtime_secs = 10;
            let id = ValidatorId::new(i as u32);
            std::thread::spawn(move || {
                let mut pv = ProcessValidator::new(id, cfg, transport);
                // Surface the real error if run() fails, instead of an opaque
                // thread-panic at join, so a flake under parallel load is
                // diagnosable rather than mysterious.
                pv.run()
                    .unwrap_or_else(|e| panic!("validator {i} run() errored: {e:?}"))
            })
        })
        .collect();

    let results: Vec<ProcessResult> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Every validator must cross the cutover and finalize past h_c.
    for r in &results {
        assert!(
            r.max_finalized_height >= 4,
            "validator {} stalled at height {} (expected >= 4)",
            r.validator,
            r.max_finalized_height
        );
        assert!(
            r.migration_success,
            "validator {} did not complete migration",
            r.validator
        );
    }

    // Cross-process agreement: no two validators may report different hashes
    // at the same height (that would be an observed fork in the no-fault run).
    let mut by_height: BTreeMap<u64, String> = BTreeMap::new();
    for r in &results {
        for (h, hash) in &r.committed {
            match by_height.get(h) {
                Some(seen) => assert_eq!(
                    seen, hash,
                    "observed fork at height {h}: validators disagree on block hash"
                ),
                None => {
                    by_height.insert(*h, hash.clone());
                }
            }
        }
    }
    assert!(
        by_height.len() >= 4,
        "expected agreement on >=4 heights, got {}",
        by_height.len()
    );
}

/// Run n=6 ProcessValidators over a partitioned shared bus (sides {0,1,2}
/// and {3,4,5}), partition engaging at h_c, and report whether an observed
/// cross-process fork occurred. `quorum_gate` toggles SAGE's quorum-gated
/// cutover. This is the in-process analogue of the `spawn_testbed` multi-OS
/// experiment, fast enough to run as a unit test.
fn run_partitioned(quorum_gate: bool) -> bool {
    use sage_network::SharedMemoryTransport;
    use std::collections::BTreeMap;

    let n = 6u32;
    let h_c = 4u64;
    // The partition engages at h_c, so the schedule's cutover height MUST be
    // h_c too: otherwise cutover completes UNPARTITIONED at an earlier height
    // (all n reach n-f and switch), then the later partition forks the
    // HotStuff phase regardless of the gate. Matching them is what makes the
    // quorum gate the decisive variable (mirrors `spawn_testbed --h-c 4`).
    let sched = Schedule {
        h_d: Height::new(1),
        h_c: Height::new(h_c),
        h_r: Height::new(8),
        kappa: 1,
        tau_blocks: 1,
    };
    let ids: Vec<ValidatorId> = (0..n).map(ValidatorId::new).collect();
    let transports = SharedMemoryTransport::mesh(&ids);
    let side_a: BTreeSet<ValidatorId> = [0, 1, 2].into_iter().map(ValidatorId::new).collect();
    let side_b: BTreeSet<ValidatorId> = [3, 4, 5].into_iter().map(ValidatorId::new).collect();

    let handles: Vec<_> = transports
        .into_iter()
        .enumerate()
        .map(|(i, transport)| {
            let mut cfg = NodeConfig::local_testnet(n, 1, sched, Height::new(5));
            cfg.max_runtime_secs = 8;
            // Short view timeout: in-process the 6 validator loops are
            // serialized behind one bus mutex, so side B's view rotation
            // (0->3 via timeout certificates) is wall-clock-gated. A short
            // timeout lets the minority side rotate to its in-partition
            // leader within the test budget; the multi-process testbed runs
            // truly parallel and forks at the default 200ms.
            cfg.view_timeout_ms = 25;
            let id = ValidatorId::new(i as u32);
            let keep = if i < 3 {
                side_a.clone()
            } else {
                side_b.clone()
            };
            std::thread::spawn(move || {
                let mut pv = ProcessValidator::new(id, cfg, transport)
                    .with_coinbase()
                    .with_partition(keep, Height::new(h_c));
                if quorum_gate {
                    pv = pv.with_cutover_quorum();
                }
                pv.run().unwrap()
            })
        })
        .collect();

    let results: Vec<ProcessResult> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Observed fork = two validators report different hashes at one height.
    let mut by_height: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
    for r in &results {
        for (h, hash) in &r.committed {
            by_height.entry(*h).or_default().insert(hash.clone());
        }
    }
    by_height.values().any(|hashes| hashes.len() > 1)
}

/// M3 differential safety result (the central artifact). Both arms run the
/// IDENTICAL infrastructure (same partition at h_c, pacemaker, coinbase,
/// tolerant handler); the ONLY difference is SAGE's quorum-gated cutover.
///
/// - Broken control (no gate, like HardFork): the partition opens a fork
///   window and a fork IS observed — proving the window is genuinely
///   reachable, so a zero-fork result is meaningful, not structural.
/// - SAGE (quorum gate): a partitioned side of 3 cannot reach the n-f=5
///   cutover quorum, so neither side switches engines and NO fork occurs.
///
/// This is the causal demonstration: same window, gate is what averts it.
#[test]
#[ignore = "timing-dependent ~16s differential experiment; run on demand with \
                `cargo test -p sage-node --lib sage_quorum_gate -- --ignored`. \
                The authoritative, parallel artifact is `spawn_testbed` (5/5 both arms)."]
fn sage_quorum_gate_prevents_partition_fork_broken_control_fires() {
    let broken_control_forks = run_partitioned(false);
    assert!(
        broken_control_forks,
        "broken control (no quorum gate) MUST fork under a 3/3 partition — \
             otherwise the fork window is not reachable and the SAGE result is vacuous"
    );

    let sage_forks = run_partitioned(true);
    assert!(
        !sage_forks,
        "SAGE (quorum-gated cutover) must NOT fork: a side of 3 < n-f=5 cannot \
             form the cutover quorum, so neither side switches engines"
    );
}

#[test]
fn four_validator_sage_migration_reaches_cutover() {
    let cfg = NodeConfig::local_testnet(4, 1, no_fault_schedule(), Height::new(3));
    let mut rt = LocalRuntime::new(cfg).unwrap();
    let m = rt.run().unwrap();
    assert!(
        m.finalized_blocks >= 3,
        "legacy path should reach cutover height"
    );
    assert!(
        m.migration_success,
        "all validators should observe SAGE cutover"
    );
    assert!(!m.safety_violation, "no safety violations");
}

#[test]
fn four_validator_sage_finalizes_after_cutover() {
    let mut cfg = NodeConfig::local_testnet(4, 1, no_fault_schedule(), Height::new(4));
    cfg.max_runtime_secs = 2;
    let mut rt = LocalRuntime::new(cfg).unwrap();
    let m = rt.run().unwrap();
    assert!(
        m.finalized_blocks >= 4,
        "target HotStuff should finalize at least one post-cutover block, got {}",
        m.finalized_blocks
    );
    assert!(
        m.migration_success,
        "migration should complete through target finality"
    );
    assert!(!m.safety_violation, "no safety violations");
}

#[test]
fn store_backed_vote_record_rejects_conflict_after_runtime_restart() {
    let cfg = NodeConfig::local_testnet(4, 1, no_fault_schedule(), Height::new(1));
    let mut rt = LocalRuntime::new(cfg).unwrap();
    let m = rt.run().unwrap();
    assert!(m.finalized_blocks >= 1);

    let engine_id = EngineId::new(EngineKind::Poa, sage_core::EngineGeneration::new(1));
    let restarted_store = rt.nodes[0].store.clone();
    let mut restarted_node_store = restarted_store;
    let err = restarted_node_store
        .put_vote(VoteRecord {
            validator: ValidatorId::new(0),
            engine_id,
            view: View::new(1),
            height: Height::new(1),
            block_hash: Hash32::new([0xEE; 32]),
        })
        .unwrap_err();
    assert!(matches!(err, sage_store::StoreError::ConflictingVote));
}

#[test]
fn restored_node_recovers_committed_height_and_state_snapshot() {
    let cfg = NodeConfig::local_testnet(4, 1, no_fault_schedule(), Height::new(2));
    let mut rt = LocalRuntime::new(cfg.clone()).unwrap();
    let m = rt.run().unwrap();
    assert!(m.finalized_blocks >= 2);

    let persisted = rt.nodes[0].store.clone();
    let expected_height = rt.nodes[0].committed.last().unwrap().block.header.height;
    let expected_root = rt.nodes[0].state.root();

    let mut restarted = LocalRuntime::new(cfg).unwrap();
    restarted.nodes[0].restore_from_store(persisted).unwrap();

    assert_eq!(
        restarted.nodes[0].next_height(),
        expected_height.checked_next().unwrap()
    );
    assert_eq!(restarted.nodes[0].state.root(), expected_root);
    assert_eq!(
        restarted.nodes[0].committed.len(),
        rt.nodes[0].committed.len()
    );
}

#[cfg(feature = "real-crypto")]
#[test]
fn persisted_cutover_decision_restores_target_authority() {
    use sage_manifest::real_ed25519::RealEd25519Scheme;
    use sage_manifest::{CutoverCertificate, SignatureScheme};

    let cfg = NodeConfig::local_testnet(4, 1, no_fault_schedule(), Height::new(4));
    let mut first = LocalRuntime::new(cfg.clone()).unwrap();
    first.run().unwrap();
    let boundary = first.nodes[0]
        .store
        .get_block(cfg.migration.h_c)
        .unwrap()
        .clone();
    let payload = first.nodes[0].cutover_payload(
        cfg.migration.h_c,
        boundary.block.hash(),
        boundary.block.header.state_root,
        first.nodes[0].target.engine_id(),
    );
    let payload_hash =
        sage_core::crypto::hash_canonical(sage_core::crypto::HashDomain::CertificateV1, &payload);
    let mut scheme = RealEd25519Scheme::new();
    let mut shares = BTreeMap::new();
    for id in 0..3 {
        let signer = ValidatorId::new(id);
        scheme.register_deterministic(signer, cfg.workload_seed);
        shares.insert(signer, scheme.sign(signer, payload_hash).unwrap());
    }
    let mut persisted = first.nodes[0].store.clone();
    persisted
        .put_migration_decision(MigrationDecisionRecord {
            version: 1,
            required_power: 3,
            cut_cert: CutoverCertificate { payload, shares },
        })
        .unwrap();

    let mut restarted = LocalRuntime::new(cfg).unwrap();
    restarted.nodes[0].restore_from_store(persisted).unwrap();
    assert_eq!(
        restarted.nodes[0].cutover_height,
        Some(restarted.config.migration.h_c)
    );
    assert_eq!(
        restarted.nodes[0].authoritative_engine(),
        restarted.nodes[0].target.engine_id()
    );
}

#[test]
fn restore_rejects_tip_snapshot_root_mismatch() {
    let cfg = NodeConfig::local_testnet(4, 1, no_fault_schedule(), Height::new(2));
    let mut first = LocalRuntime::new(cfg.clone()).unwrap();
    first.run().unwrap();
    let mut persisted = first.nodes[0].store.clone();
    let highest = persisted.highest_block().unwrap();
    let bytes = persisted.get_state(highest).unwrap().to_vec();
    persisted
        .put_state(highest, Hash32::new([0xAB; 32]), bytes)
        .unwrap();

    let mut restarted = LocalRuntime::new(cfg).unwrap();
    assert!(restarted.nodes[0].restore_from_store(persisted).is_err());
    assert!(restarted.nodes[0].committed.is_empty() || restarted.nodes[0].cutover_height.is_none());
}

#[test]
fn restored_runtime_continues_consensus_after_restart() {
    let mut first_cfg = NodeConfig::local_testnet(4, 1, no_fault_schedule(), Height::new(2));
    first_cfg.max_runtime_secs = 2;
    let mut first = LocalRuntime::new(first_cfg.clone()).unwrap();
    let before = first.run().unwrap();
    assert!(before.finalized_blocks >= 2);

    let stores: Vec<MemoryBackend> = first.nodes.iter().map(|node| node.store.clone()).collect();
    let restored_roots: Vec<_> = first.nodes.iter().map(|node| node.state.root()).collect();
    let restored_lengths: Vec<_> = first
        .nodes
        .iter()
        .map(|node| node.committed.len())
        .collect();

    let mut second_cfg = first_cfg;
    second_cfg.max_height = Height::new(4);
    second_cfg.max_runtime_secs = 2;
    let mut second = LocalRuntime::new(second_cfg).unwrap();
    for (idx, store) in stores.into_iter().enumerate() {
        second.restore_node_from_store(idx, store).unwrap();
        assert_eq!(second.nodes[idx].state.root(), restored_roots[idx]);
        assert_eq!(second.nodes[idx].committed.len(), restored_lengths[idx]);
    }

    let after = second.run().unwrap();
    assert!(
        after.finalized_blocks >= 4,
        "restored runtime should continue to target height, got {}",
        after.finalized_blocks
    );
    assert!(
        !after.safety_violation,
        "no safety violations after restart"
    );
}
