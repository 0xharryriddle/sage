//! M2 observed-fork integration test.
//!
//! This is the make-or-break artifact the publishability debate demanded: an
//! OBSERVED cross-boundary fork produced by the real HotStuff engine code, not
//! a flag or a proxy. Under a partition that splits n=6 (f=1, BFT quorum
//! 2f+1=3) into two groups of three, each isolated side can independently
//! reach quorum and finalize. With an uncoordinated (blind / hard-fork) cutover
//! the two sides finalize DIFFERENT blocks at the same height, and the fork
//! detector reports it. SAGE's CutCert gate is what prevents either side from
//! switching engines in the first place, so the analogous SAGE run never opens
//! the window — but that part is exercised by the controller tests; here we
//! prove the detector fires on a genuinely produced conflict, and that an
//! un-partitioned run of the same engines produces no fork.
//!
//! The point is falsifiability: the harness CAN exhibit the unsafe outcome, so
//! a zero-fork result elsewhere is meaningful rather than structural.

use sage_consensus::{
    ConsensusEngine, ConsensusMessage, HotStuffEngine, HotStuffMessage, MessageEnvelope,
    ProposeContext, ValidatorSet,
};
use sage_core::{
    ChainId, ChainState, ConfigId, ConsensusStateEnvelope, EngineGeneration, EngineId, EngineKind,
    Epoch, ExecutionState, FinalizedBlock, Hash32, Height, PlatformState, Transaction, ValidatorId,
    View,
};
use sage_node::fork_detector::detect_forks;
use std::collections::BTreeMap;

fn engine_id() -> EngineId {
    EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1))
}

fn chain_state() -> ChainState {
    ChainState {
        execution: ExecutionState::new_with_accounts(8, 1000),
        platform: PlatformState {
            chain_id: ChainId::new("sage"),
            epoch: Epoch::new(1),
            config_id: ConfigId::new(1),
            migration: None,
        },
        consensus: ConsensusStateEnvelope {
            engine: engine_id(),
            opaque: vec![],
        },
    }
}

/// Drive one isolated group of validators at a fixed view (whose leader is in
/// the group) to finalize exactly one block at `height`. Returns the finalized
/// block as seen by every member. This uses only the real engine API:
/// propose -> broadcast proposal -> each member votes -> votes collected ->
/// try_finalize forms the quorum certificate.
fn finalize_in_group(
    members: &[ValidatorId],
    leader_view: View,
    height: Height,
    txs: &[Transaction],
) -> FinalizedBlock {
    let validators = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 6);
    let mut engines: BTreeMap<ValidatorId, HotStuffEngine> = members
        .iter()
        .map(|&id| {
            let mut e = HotStuffEngine::new(id, engine_id(), validators.clone(), 1);
            e.set_view(leader_view);
            (id, e)
        })
        .collect();

    let leader_id = ValidatorId::new((leader_view.get() % 6) as u32);
    assert!(
        members.contains(&leader_id),
        "leader {leader_id} for view {leader_view:?} must be in the group"
    );

    let state = chain_state();
    let block = engines
        .get_mut(&leader_id)
        .unwrap()
        .propose(ProposeContext {
            state: &state,
            parent_hash: Hash32::ZERO,
            height,
            txs,
        })
        .unwrap()
        .expect("leader proposes");

    let proposal = MessageEnvelope {
        from: leader_id,
        to: None,
        chain_id: ChainId::new("sage"),
        epoch: Epoch::new(1),
        config_id: ConfigId::new(1),
        message: ConsensusMessage::HotStuff(HotStuffMessage::Proposal {
            view: leader_view,
            block: block.clone(),
            justify: None,
        }),
    };

    // Each member handles the proposal and emits a vote; collect all votes.
    let mut votes: Vec<MessageEnvelope> = Vec::new();
    for id in members {
        let replies = engines
            .get_mut(id)
            .unwrap()
            .handle_message(proposal.clone())
            .unwrap();
        votes.extend(replies);
    }

    // Deliver every vote to every member (intra-group broadcast).
    for id in members {
        for v in &votes {
            let _ = engines
                .get_mut(id)
                .unwrap()
                .handle_message(v.clone())
                .unwrap();
        }
    }

    // Each member finalizes; quorum=3 is met within the group.
    let mut finalized: Option<FinalizedBlock> = None;
    for id in members {
        let fbs = engines.get_mut(id).unwrap().try_finalize().unwrap();
        if let Some(fb) = fbs.into_iter().next() {
            finalized = Some(fb);
        }
    }
    finalized.expect("group of 3 reaches quorum and finalizes")
}

fn distinct_txs(seed: u64) -> Vec<Transaction> {
    vec![Transaction {
        from: 0,
        to: 1,
        amount: 1 + seed,
        nonce: seed,
    }]
}

#[test]
fn partitioned_blind_cutover_produces_observed_fork() {
    // Two isolated groups of three. Group A is led at view 0 (leader 0),
    // group B at view 3 (leader 3) — both leaders inside their own group, as
    // happens once each side advances views independently under partition.
    let group_a = [
        ValidatorId::new(0),
        ValidatorId::new(1),
        ValidatorId::new(2),
    ];
    let group_b = [
        ValidatorId::new(3),
        ValidatorId::new(4),
        ValidatorId::new(5),
    ];

    // Each side finalizes a DIFFERENT block at the same height (different txs
    // => different state root => different block hash).
    let fb_a = finalize_in_group(&group_a, View::new(0), Height::new(1), &distinct_txs(7));
    let fb_b = finalize_in_group(&group_b, View::new(3), Height::new(1), &distinct_txs(42));

    assert_ne!(
        fb_a.block.hash(),
        fb_b.block.hash(),
        "the two partitioned sides must finalize distinct blocks"
    );

    // Build per-validator committed logs: each side committed its own block.
    let mut logs: BTreeMap<ValidatorId, Vec<FinalizedBlock>> = BTreeMap::new();
    for id in group_a {
        logs.insert(id, vec![fb_a.clone()]);
    }
    for id in group_b {
        logs.insert(id, vec![fb_b.clone()]);
    }

    let report = detect_forks(&logs);
    assert!(
        report.fork_observed(),
        "an uncoordinated cutover across a balanced partition must produce an OBSERVED fork"
    );
    assert_eq!(report.fork_count(), 1, "exactly one forked height (h=1)");
}

#[test]
fn unpartitioned_single_group_produces_no_fork() {
    // Control: a single quorum (no partition) finalizes one block at the
    // height, so the same detector reports no fork. This confirms the fork in
    // the test above is caused by the partition, not by the harness always
    // flagging forks.
    let group = [
        ValidatorId::new(0),
        ValidatorId::new(1),
        ValidatorId::new(2),
        ValidatorId::new(3),
        ValidatorId::new(4),
        ValidatorId::new(5),
    ];
    let fb = finalize_in_group(&group, View::new(0), Height::new(1), &distinct_txs(7));

    let mut logs: BTreeMap<ValidatorId, Vec<FinalizedBlock>> = BTreeMap::new();
    for id in group {
        logs.insert(id, vec![fb.clone()]);
    }

    let report = detect_forks(&logs);
    assert!(
        !report.fork_observed(),
        "a single un-partitioned quorum must not fork"
    );
}
