//! SAGE local multi-validator runtime.
//!
//! Builds on `sage-sim`'s validator logic but drives finalization via a
//! wall-clock polling loop rather than a deterministic event queue.  The
//! HotStuff pacemaker (timeout certificates, view advancement) is engaged
//! so that the real-node testbed exercises the full protocol.
//!
//! Robustness gate (P1-D): the runtime/consensus path must not panic. We deny
//! `unwrap`/`expect` in non-test builds; test code may still use them freely.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
pub mod config;
pub mod cutover_gate;
pub mod error;
pub mod fork_detector;
pub mod metrics;
pub mod process_result;

use config::NodeConfig;
use sage_consensus::{
    ConsensusEngine, ConsensusMessage, HotStuffEngine, HotStuffMessage, MessageEnvelope, PoaEngine,
    ProposeContext, SageMessage, ValidatorSet,
};
use sage_controller::{
    ReadinessContext, ReplayBlockContext, ReplayContext, SageController, ShadowRecord,
};
use sage_core::{
    Block, ChainState, ConsensusStateEnvelope, EngineId, EngineKind, ExecutionState,
    FinalizedBlock, Hash32, Height, PlatformState, ValidatorId, View,
};
use sage_network::{InMemoryTransport, Transport};
use sage_store::{
    BlockStore, CommittedTransition, MemoryBackend, MigrationDecisionRecord, MigrationStore,
    SafetyStore, StateStore, TransactionalStore, VoteRecord,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub use error::{NodeError, NodeResult};

pub use metrics::NodeMetrics;

/// Exact identity of a signed cutover statement. Quorum accounting must use
/// every signed field; otherwise shares for different decisions could combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CutoverKey {
    height: Height,
    boundary_block: Hash32,
    boundary_root: sage_core::StateRoot,
    target_engine: EngineId,
}

// ---------------------------------------------------------------------------
// NodeValidator  (thin wrapper around engines + controller)
// ---------------------------------------------------------------------------

struct NodeValidator {
    id: ValidatorId,
    legacy: PoaEngine,
    target: HotStuffEngine,
    controller: SageController,
    state: ChainState,
    committed: Vec<FinalizedBlock>,
    cutover_height: Option<Height>,
    strategy: StrategyKind,
    store: MemoryBackend,
    /// Monotonic per-view pacemaker deadline (research Bug C fix). A node only
    /// emits a HotStuff timeout once it has spent real wall-clock time in a
    /// view WITHOUT progress. Without this, every tick emits a timeout, 2f+1
    /// timeouts form a TC that clears pending_votes before the lone leader's
    /// votes reach quorum, and the view races away (observed at n>=8).
    view_deadline: Option<Instant>,
    /// View the current deadline was armed for; a change means we entered a new
    /// view and must re-arm.
    deadline_view: u64,
    /// Consecutive timeouts without progress, for exponential backoff so the
    /// timeout eventually exceeds real message-delivery latency at high n.
    consecutive_timeouts: u32,
    /// QUORUM-GATED CUTOVER (SAGE safety enforcement). When `Some(threshold)`,
    /// reaching LOCAL readiness does NOT switch engines; instead the node emits a
    /// cutover attestation and only switches once `threshold` (= n-f) DISTINCT
    /// validators have attested the same boundary. `None` preserves the legacy
    /// local-switch behavior used by the single-process LocalRuntime and the
    /// deterministic simulator. Two disjoint sets of size >= n-f require
    /// 2(n-f) <= n i.e. n <= 2f, contradicting BFT n >= 2f+1, so under any
    /// partition at most one side can form the quorum and switch — no fork.
    cutover_quorum: Option<u64>,
    /// Boundary this node is locally ready to cut over at, awaiting quorum:
    /// (cutover_height, boundary_block_hash, boundary_state_root, cert_hash).
    pending_cutover: Option<(Height, Hash32, sage_core::StateRoot, Hash32)>,
    /// Distinct attesters per exact signed payload, including this node once it
    /// is locally ready. Early attestations remain payload-isolated.
    cutover_attesters: std::collections::BTreeMap<
        CutoverKey,
        BTreeMap<ValidatorId, sage_manifest::SignatureEnvelope>,
    >,
    /// When true, each proposed block carries a proposer-beneficiary
    /// ("coinbase") transaction so block content depends on WHO proposed it.
    /// Enabled only on the multi-process `ProcessValidator` path, where two
    /// partitioned leaders proposing at the same height must produce DISTINCT
    /// block hashes for the fork detector to fire. The single-process
    /// `LocalRuntime` and the simulator keep it OFF: they have one global
    /// proposer per height (no competing blocks) and mutating state via coinbase
    /// would change persisted roots, breaking the restart-continuation test.
    coinbase_enabled: bool,
    /// Synthetic transactions minted per block for the throughput measurement
    /// (height-derived, proposer-independent). 0 preserves empty-block behavior.
    workload_txs_per_block: u64,
    /// Configured committee used to reject out-of-domain envelopes before they
    /// mutate consensus or migration-control state.
    committee: BTreeSet<ValidatorId>,
    /// Testbed-only deterministic key seed. Production deployments must load
    /// independently provisioned validator keys instead.
    attestation_key_seed: u64,
}

pub use config::NodeStrategy as StrategyKind;

impl NodeValidator {
    fn new(
        id: ValidatorId,
        cfg: &NodeConfig,
        state: ChainState,
        legacy: PoaEngine,
        target: HotStuffEngine,
        controller: SageController,
    ) -> Self {
        Self {
            id,
            legacy,
            target,
            controller,
            state,
            committed: Vec::new(),
            cutover_height: None,
            strategy: cfg.strategy,
            store: MemoryBackend::new(),
            view_deadline: None,
            deadline_view: 0,
            consecutive_timeouts: 0,
            cutover_quorum: None,
            pending_cutover: None,
            cutover_attesters: std::collections::BTreeMap::new(),
            coinbase_enabled: false,
            workload_txs_per_block: cfg.workload_txs_per_block,
            committee: cfg.validator_ids().into_iter().collect(),
            attestation_key_seed: cfg.workload_seed,
        }
    }

    fn next_height(&self) -> Height {
        self.committed
            .last()
            .map(|b| b.block.header.height)
            .unwrap_or(Height::new(0))
            .checked_next()
            .unwrap_or(Height::new(1))
    }

    fn restore_from_store(&mut self, store: MemoryBackend) -> NodeResult<()> {
        self.store = store;
        self.committed.clear();
        let Some(highest) = self.store.highest_block() else {
            return Ok(());
        };

        for h in 1..=highest.get() {
            let height = Height::new(h);
            let block = self.store.get_block(height)?.clone();
            self.committed.push(block);
        }

        let bytes = self.store.get_state(highest)?;
        let restored_state: ChainState = serde_json::from_slice(bytes)?;
        let stored_root = self.store.state_root(highest).ok_or_else(|| {
            NodeError::Consensus("committed tip is missing its persisted state root".into())
        })?;
        let tip = self.store.get_block(highest)?;
        if restored_state.root() != stored_root || tip.block.header.state_root != stored_root {
            return Err(NodeError::Consensus(
                "committed tip block, snapshot, and stored state root disagree".into(),
            ));
        }
        self.state = restored_state;
        self.restore_migration_decision()?;
        Ok(())
    }

    #[cfg(feature = "real-crypto")]
    fn restore_migration_decision(&mut self) -> NodeResult<()> {
        use sage_consensus::QuorumThreshold;
        use sage_manifest::real_ed25519::RealEd25519Scheme;

        let Some(record) = self.store.migration_decision().cloned() else {
            return Ok(());
        };
        if record.version != 1 {
            return Err(NodeError::Consensus(format!(
                "unsupported migration decision version {}",
                record.version
            )));
        }
        let payload = &record.cut_cert.payload;
        if payload.chain_id != self.state.platform.chain_id
            || payload.epoch != self.state.platform.epoch
            || payload.config_id != self.state.platform.config_id
            || payload.engine_id != self.target.engine_id()
        {
            return Err(NodeError::Consensus(
                "persisted migration decision is outside the local domain".into(),
            ));
        }
        let boundary_hash = payload.block_hash.ok_or_else(|| {
            NodeError::Consensus("persisted cutover certificate has no boundary hash".into())
        })?;
        let boundary = self.store.get_block(payload.height)?;
        if boundary.block.hash() != boundary_hash
            || boundary.block.header.state_root != payload.root
        {
            return Err(NodeError::Consensus(
                "persisted migration decision disagrees with committed boundary".into(),
            ));
        }

        let validators = ValidatorSet::equal_power(
            self.state.platform.config_id,
            self.state.platform.epoch,
            self.committee.len() as u32,
        );
        let mut scheme = RealEd25519Scheme::new();
        for signer in &self.committee {
            scheme.register_deterministic(*signer, self.attestation_key_seed);
        }
        record
            .cut_cert
            .verify(
                &validators,
                QuorumThreshold {
                    required_power: record.required_power,
                },
                &scheme,
            )
            .map_err(|err| {
                NodeError::Consensus(format!("invalid persisted cutover certificate: {err}"))
            })?;

        self.cutover_height = Some(payload.height);
        self.bootstrap_target(payload.height, payload.root, record.cut_cert.payload_hash());
        Ok(())
    }

    #[cfg(not(feature = "real-crypto"))]
    fn restore_migration_decision(&mut self) -> NodeResult<()> {
        if self.store.migration_decision().is_some() {
            return Err(NodeError::Consensus(
                "cannot restore a cutover decision without real crypto".into(),
            ));
        }
        Ok(())
    }

    fn persist_finalized(&mut self, block: &FinalizedBlock) -> NodeResult<()> {
        let bytes = serde_json::to_vec(&self.state)?;
        self.store.commit_transition(CommittedTransition {
            block: block.clone(),
            state_root: self.state.root(),
            state: bytes,
            migration_decision: None,
        })?;
        Ok(())
    }

    fn authoritative_engine(&self) -> EngineId {
        let next = self.next_height();
        match self.strategy {
            // SAGE and faithful Cox both switch only after their quorum gate
            // decides `cutover_height`; the ONLY difference between them is the
            // threshold (n-f vs 2f+1), set in `with_cutover_quorum`.
            StrategyKind::Sage | StrategyKind::CoxFaithful => {
                if self.cutover_height.map(|h| next >= h).unwrap_or(false) {
                    self.target.engine_id()
                } else {
                    self.legacy.engine_id()
                }
            }
            StrategyKind::HardFork | StrategyKind::SageBlind | StrategyKind::CoxStyle => {
                if next >= self.controller.schedule.h_c {
                    self.target.engine_id()
                } else {
                    self.legacy.engine_id()
                }
            }
            StrategyKind::StopTheWorld => {
                if next >= self.controller.schedule.h_r {
                    self.target.engine_id()
                } else {
                    self.legacy.engine_id()
                }
            }
            StrategyKind::ReconfigOnly => self.legacy.engine_id(),
        }
    }

    fn propose(&mut self, height: Height) -> NodeResult<Option<Block>> {
        self.propose_variant(height, 1)
    }

    /// Propose a block whose proposer-coinbase mints `variant` units. A distinct
    /// `variant` yields a distinct block hash at the same height/parent, which is
    /// exactly what an equivocating Byzantine leader needs: it mints two blocks
    /// (variant 1 and variant 2) for one height and sends each to a disjoint set
    /// of peers. With `coinbase_enabled` off the txs are empty and `variant` has
    /// no effect, so honest no-fault runs are unchanged.
    fn propose_variant(&mut self, height: Height, variant: u64) -> NodeResult<Option<Block>> {
        let parent = self
            .committed
            .last()
            .map(|b| b.block.hash())
            .unwrap_or(Hash32::ZERO);
        // Proposer-beneficiary ("coinbase") transaction: mint a unit to the
        // PROPOSER's account from a treasury sentinel. This is faithful to real
        // chains (every block carries a proposer reward) and is what makes a
        // block's content depend on WHO proposed it. Without it, two different
        // leaders proposing at the same height with the same parent and empty
        // txs would produce a BYTE-IDENTICAL block hash, so the fork detector
        // could not fire even on a genuine cross-partition double-finalization
        // (the "fork impossible by construction" artifact the publishability
        // review flagged). In the honest case there is one proposer per height,
        // so all nodes still agree; only under a partition do two leaders mint
        // distinct coinbases at the same height -> distinct hashes -> an
        // OBSERVED fork.
        let coinbase = sage_core::Transaction {
            from: u64::MAX, // treasury sentinel (not a funded account)
            to: self.id.get() as u64,
            amount: variant,
            nonce: height.get(),
        };
        // Synthetic throughput workload: `workload_txs_per_block` transactions
        // whose sender/receiver/amount are derived ONLY from the height (not the
        // proposer), so every correct proposer at a given height mints a
        // byte-identical block and the no-fault path stays fork-free while
        // carrying a real, non-zero transaction count for the TPS measurement.
        //
        // The txs are emitted as BALANCED round-trip PAIRS (a->b then b->a, unit
        // amount, on funded accounts), so each block is state-root NEUTRAL: after
        // applying a block the account balances return exactly to their prior
        // values, hence the committed state root is height-invariant. This is
        // deliberate. The testbed advances execution state only on FINALIZATION
        // and does not maintain a speculative per-block state tree, so a
        // state-CHANGING workload would make the proposal's state root depend on
        // the (possibly pipelined / reordered) in-flight prefix and diverge from
        // a validator's recomputation under message reordering — a testbed
        // execution-model artifact unrelated to SAGE's consensus/migration
        // contribution. A neutral workload decouples the throughput measurement
        // from that artifact: the transaction COUNT and per-tx execution cost are
        // real (every tx is applied), only the net balance delta per block is
        // zero. Accounts 0..FUNDED are seeded (see genesis new_with_accounts), so
        // unit transfers never underflow and each pair self-cancels.
        let mut synthetic: Vec<sage_core::Transaction> = Vec::new();
        if self.workload_txs_per_block > 0 {
            const FUNDED: u64 = 10; // matches genesis new_with_accounts(10, 1000)
                                    // Emit floor(N/2) balanced pairs => 2*floor(N/2) real txs. An odd
                                    // requested count is rounded down to keep every block net-zero.
            let pairs = self.workload_txs_per_block / 2;
            for k in 0..pairs {
                let base = height.get().wrapping_mul(pairs).wrapping_add(k);
                let a = base % FUNDED;
                let b = (base + 1) % FUNDED; // a != b since consecutive mod FUNDED
                let nonce = base.wrapping_mul(2);
                synthetic.push(sage_core::Transaction {
                    from: a,
                    to: b,
                    amount: 1,
                    nonce,
                });
                synthetic.push(sage_core::Transaction {
                    from: b,
                    to: a,
                    amount: 1,
                    nonce: nonce.wrapping_add(1),
                });
            }
        }
        let txs: Vec<sage_core::Transaction> = if self.coinbase_enabled {
            std::iter::once(coinbase).chain(synthetic).collect()
        } else {
            synthetic
        };
        let ctx = ProposeContext {
            height,
            parent_hash: parent,
            state: &self.state,
            txs: &txs,
        };
        if self.authoritative_engine().kind == EngineKind::Poa {
            Ok(self.legacy.propose(ctx)?)
        } else {
            Ok(self.target.propose(ctx)?)
        }
    }

    fn handle_message(&mut self, msg: MessageEnvelope) -> NodeResult<Vec<MessageEnvelope>> {
        self.validate_envelope(&msg)?;
        let replies = match msg.message {
            ConsensusMessage::Poa(_) => self.legacy.handle_message(msg)?,
            ConsensusMessage::HotStuff(_) => {
                let _ = self.legacy.handle_message(msg.clone());
                self.target.handle_message(msg)?
            }
            ConsensusMessage::Sage(SageMessage::CutoverAttestation {
                height,
                boundary_block,
                boundary_root,
                target_engine,
                signature,
            }) => {
                let authenticated_share = self.verify_cutover_attestation(
                    msg.from,
                    height,
                    boundary_block,
                    boundary_root,
                    target_engine,
                    &signature,
                )?;
                self.cutover_attesters
                    .entry(CutoverKey {
                        height,
                        boundary_block,
                        boundary_root,
                        target_engine,
                    })
                    .or_default()
                    .insert(msg.from, authenticated_share);
                Vec::new()
            }
            // This multi-process runtime is concretely PoA->HotStuff (see the
            // `legacy: PoaEngine` / `target: HotStuffEngine` fields); Raft is a
            // conforming legacy engine exercised in the deterministic path and
            // its own unit tests, not this testbed, so its traffic is inert here.
            ConsensusMessage::Raft(_) => Vec::new(),
        };
        for reply in &replies {
            self.persist_local_vote(reply)?;
        }
        Ok(replies)
    }

    fn validate_envelope(&self, msg: &MessageEnvelope) -> NodeResult<()> {
        let platform = &self.state.platform;
        if msg.to.is_some_and(|to| to != self.id)
            || msg.chain_id != platform.chain_id
            || msg.epoch != platform.epoch
            || msg.config_id != platform.config_id
            || !self.committee.contains(&msg.from)
        {
            return Err(NodeError::Consensus(format!(
                "rejected out-of-domain envelope from validator {}",
                msg.from.get()
            )));
        }
        Ok(())
    }

    fn cutover_payload(
        &self,
        height: Height,
        boundary_block: Hash32,
        boundary_root: sage_core::StateRoot,
        target_engine: EngineId,
    ) -> sage_manifest::certificate::CertificatePayload {
        sage_manifest::certificate::CertificatePayload {
            chain_id: self.state.platform.chain_id.clone(),
            epoch: self.state.platform.epoch,
            config_id: self.state.platform.config_id,
            kind: sage_manifest::certificate::CertificateKind::Cutover,
            height,
            root: boundary_root,
            block_hash: Some(boundary_block),
            engine_id: target_engine,
        }
    }

    #[cfg(feature = "real-crypto")]
    fn sign_cutover_attestation(
        &self,
        height: Height,
        boundary_block: Hash32,
        boundary_root: sage_core::StateRoot,
        target_engine: EngineId,
    ) -> NodeResult<Vec<u8>> {
        use sage_manifest::real_ed25519::RealEd25519Scheme;
        use sage_manifest::{SignatureEnvelope, SignatureScheme};

        let payload = self.cutover_payload(height, boundary_block, boundary_root, target_engine);
        let hash = sage_core::crypto::hash_canonical(
            sage_core::crypto::HashDomain::CertificateV1,
            &payload,
        );
        let mut scheme = RealEd25519Scheme::new();
        scheme.register_deterministic(self.id, self.attestation_key_seed);
        match scheme.sign(self.id, hash) {
            Ok(SignatureEnvelope::Ed25519 {
                signature_bytes, ..
            }) => Ok(signature_bytes),
            Ok(_) => Err(NodeError::Consensus(
                "unexpected cutover signature scheme".into(),
            )),
            Err(err) => Err(NodeError::Consensus(err.to_string())),
        }
    }

    #[cfg(feature = "real-crypto")]
    fn verify_cutover_attestation(
        &self,
        signer: ValidatorId,
        height: Height,
        boundary_block: Hash32,
        boundary_root: sage_core::StateRoot,
        target_engine: EngineId,
        signature: &[u8],
    ) -> NodeResult<sage_manifest::SignatureEnvelope> {
        use sage_manifest::real_ed25519::RealEd25519Scheme;
        use sage_manifest::{SignatureEnvelope, SignatureScheme};

        if target_engine != self.target.engine_id() {
            return Err(NodeError::Consensus(
                "cutover attestation targets wrong engine".into(),
            ));
        }
        let payload = self.cutover_payload(height, boundary_block, boundary_root, target_engine);
        let hash = sage_core::crypto::hash_canonical(
            sage_core::crypto::HashDomain::CertificateV1,
            &payload,
        );
        let mut scheme = RealEd25519Scheme::new();
        scheme.register_deterministic(signer, self.attestation_key_seed);
        let envelope = SignatureEnvelope::Ed25519 {
            signer,
            payload_hash: hash,
            signature_bytes: signature.to_vec(),
        };
        let signers = BTreeSet::from([signer]);
        scheme
            .verify(&signers, hash, &envelope)
            .map_err(|err| NodeError::Consensus(format!("invalid cutover attestation: {err}")))?;
        Ok(envelope)
    }

    #[cfg(not(feature = "real-crypto"))]
    fn sign_cutover_attestation(
        &self,
        _height: Height,
        _boundary_block: Hash32,
        _boundary_root: sage_core::StateRoot,
        _target_engine: EngineId,
    ) -> NodeResult<Vec<u8>> {
        Err(NodeError::Consensus(
            "quorum cutover requires the real-crypto feature".into(),
        ))
    }

    #[cfg(not(feature = "real-crypto"))]
    fn verify_cutover_attestation(
        &self,
        _signer: ValidatorId,
        _height: Height,
        _boundary_block: Hash32,
        _boundary_root: sage_core::StateRoot,
        _target_engine: EngineId,
        _signature: &[u8],
    ) -> NodeResult<sage_manifest::SignatureEnvelope> {
        Err(NodeError::Consensus(
            "quorum cutover requires the real-crypto feature".into(),
        ))
    }

    /// Like `handle_message`, but swallows BENIGN BFT protocol rejections that
    /// are normal operation rather than runtime faults — a node legitimately
    /// refuses to double-vote in a view, ignores a stale-view message, or
    /// rejects a proposal from a non-leader. Under a partition view-change these
    /// fire routinely (a rotated side replays/duplicates messages while
    /// converging). Propagating them via `?` would KILL the validator process —
    /// observed: side B's view-3 leader crashed with `DoubleVoteAttempt`, which
    /// both made the broken-control fork flaky and confounded SAGE's zero-fork
    /// result (a dead leader cannot fork regardless of strategy). Real errors
    /// (overflow, unknown validator) still propagate.
    fn handle_message_tolerant(
        &mut self,
        msg: MessageEnvelope,
    ) -> NodeResult<Vec<MessageEnvelope>> {
        self.validate_envelope(&msg)?;
        // SAGE cutover attestations are not engine traffic — record them toward
        // the quorum via the normal handler (which never errors on them).
        if matches!(msg.message, ConsensusMessage::Sage(_)) {
            return self.handle_message(msg);
        }
        let result = match msg.message {
            ConsensusMessage::Poa(_) => self.legacy.handle_message(msg),
            ConsensusMessage::HotStuff(_) => {
                let _ = self.legacy.handle_message(msg.clone());
                self.target.handle_message(msg)
            }
            ConsensusMessage::Sage(_) => unreachable!("handled above"),
            // PoA->HotStuff runtime: Raft traffic is inert here (see handle_message).
            ConsensusMessage::Raft(_) => Ok(Vec::new()),
        };
        let replies = match result {
            Ok(replies) => replies,
            Err(
                sage_consensus::ConsensusError::DoubleVoteAttempt { .. }
                | sage_consensus::ConsensusError::StaleView { .. }
                | sage_consensus::ConsensusError::InvalidProposal { .. },
            ) => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        for reply in &replies {
            self.persist_local_vote(reply)?;
        }
        Ok(replies)
    }

    fn persist_local_vote(&mut self, msg: &MessageEnvelope) -> NodeResult<()> {
        match &msg.message {
            ConsensusMessage::Poa(sage_consensus::PoaMessage::Vote { block, block_hash }) => {
                self.store.put_vote(VoteRecord {
                    validator: self.id,
                    engine_id: block.header.engine_id,
                    view: View::new(block.header.height.get()),
                    height: block.header.height,
                    block_hash: *block_hash,
                })?;
            }
            ConsensusMessage::HotStuff(HotStuffMessage::Vote {
                view,
                block,
                block_hash,
            }) => {
                self.store.put_vote(VoteRecord {
                    validator: self.id,
                    engine_id: block.header.engine_id,
                    view: *view,
                    height: block.header.height,
                    block_hash: *block_hash,
                })?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Decide cutover for non-SAGE strategies; returns cutover height if one
    /// was just decided.
    fn decide_cutover(&mut self, height: Height) -> Option<Height> {
        let schedule = self.controller.schedule;
        match self.strategy {
            // SAGE and faithful Cox decide cutover via the quorum-gated path
            // (drive_quorum_cutover), not the schedule, so decide_cutover is a
            // no-op for both.
            StrategyKind::Sage | StrategyKind::CoxFaithful => None,
            StrategyKind::HardFork => {
                if height.get() >= schedule.h_c.get() && self.cutover_height.is_none() {
                    self.cutover_height = Some(schedule.h_c);
                    Some(schedule.h_c)
                } else {
                    None
                }
            }
            StrategyKind::SageBlind => {
                if height.get() >= schedule.h_c.get() && self.cutover_height.is_none() {
                    self.cutover_height = Some(schedule.h_c);
                    Some(schedule.h_c)
                } else {
                    None
                }
            }
            StrategyKind::CoxStyle => {
                // Cox-style live switch at h_c: forms a boundary checkpoint and
                // switches with no halt, but WITHOUT SAGE's n-f dual-run cutover
                // quorum gate. Under a partition each side that reaches the
                // switch height swaps engines locally, so the safety consequence
                // of the absent gate is OBSERVED by the multi-process testbed.
                if height.get() >= schedule.h_c.get() && self.cutover_height.is_none() {
                    self.cutover_height = Some(schedule.h_c);
                    Some(schedule.h_c)
                } else {
                    None
                }
            }
            StrategyKind::StopTheWorld => {
                if height.get() >= schedule.h_r.get() && self.cutover_height.is_none() {
                    self.cutover_height = Some(schedule.h_r);
                    Some(schedule.h_r)
                } else {
                    None
                }
            }
            StrategyKind::ReconfigOnly => None,
        }
    }

    fn bootstrap_target(
        &mut self,
        at_height: Height,
        root: sage_core::StateRoot,
        cert_hash: Hash32,
    ) {
        let _ = self.target.init(sage_consensus::BootstrapAnchor {
            height: at_height,
            root,
            certificate_hash: cert_hash,
        });
        self.target.set_view(View::new(0));
    }

    fn try_finalize(
        &mut self,
        view_timeout_ms: u64,
    ) -> NodeResult<(Vec<FinalizedBlock>, Vec<MessageEnvelope>)> {
        let _next = self.next_height();
        let is_legacy = self.authoritative_engine().kind == EngineKind::Poa;

        let legacy_finalized = self.legacy.try_finalize()?;
        let (finalized, pacemaker_msgs) = if is_legacy {
            (legacy_finalized, Vec::new())
        } else {
            let target_finalized = self.target.try_finalize()?;
            if target_finalized.is_empty() {
                // Pacemaker: try to advance view via TC, or emit timeout —
                // but ONLY after a monotonic per-view deadline elapses (research
                // Bug C fix). Without the deadline, every poll emits a timeout,
                // 2f+1 timeouts form a TC that clears pending_votes before the
                // lone leader's votes reach quorum, and the view races away
                // (observed: view 0 -> 100+ at n>=8 with vote count stuck at 0).
                let chain_id = self.state.platform.chain_id.clone();
                let epoch = self.state.platform.epoch;
                let config_id = self.state.platform.config_id;

                let cur_view = self.target.current_view().get();
                let now = Instant::now();

                // Re-arm the deadline whenever we enter a new view. Back off
                // exponentially on consecutive failed views (capped) so the
                // timeout eventually exceeds real message-delivery latency at
                // high validator counts.
                if self.view_deadline.is_none() || self.deadline_view != cur_view {
                    let backoff = 1u64 << self.consecutive_timeouts.min(4); // 1,2,4,8,16
                    let dur = Duration::from_millis(view_timeout_ms.saturating_mul(backoff));
                    self.view_deadline = Some(now + dur);
                    self.deadline_view = cur_view;
                }

                let mut msgs = Vec::new();

                // Always try to advance via a TC that already formed — applying
                // a TC is progress, not a fault.
                if let Some((_new_view, msg)) = self.target.try_advance_view()? {
                    msgs.push(MessageEnvelope {
                        from: self.id,
                        to: None,
                        chain_id: chain_id.clone(),
                        epoch,
                        config_id,
                        message: msg,
                    });
                }

                // Only emit a timeout once the monotonic deadline for THIS view
                // has elapsed, giving the proposal + votes time to complete.
                let deadline_elapsed = self.view_deadline.map(|d| now >= d).unwrap_or(false);
                if deadline_elapsed {
                    if let Some(msg) = self.target.emit_timeout() {
                        msgs.push(MessageEnvelope {
                            from: self.id,
                            to: None,
                            chain_id,
                            epoch,
                            config_id,
                            message: msg,
                        });
                        self.consecutive_timeouts = self.consecutive_timeouts.saturating_add(1);
                        // Re-arm with backoff for the next interval in this view.
                        let backoff = 1u64 << self.consecutive_timeouts.min(4);
                        let dur = Duration::from_millis(view_timeout_ms.saturating_mul(backoff));
                        self.view_deadline = Some(now + dur);
                    }
                }
                // Return early — no blocks to commit yet
                return Ok((Vec::new(), msgs));
            }
            // Progress: a block finalized. Reset the pacemaker backoff and
            // disarm the deadline so the next view starts with the base timeout.
            self.consecutive_timeouts = 0;
            self.view_deadline = None;
            let mut combined = legacy_finalized;
            combined.extend(target_finalized);
            (combined, Vec::new())
        };

        // Process finalized blocks
        for block in &finalized {
            let h = block.block.header.height;
            // Decide cutover for non-SAGE strategies
            if let Some(cut_h) = self.decide_cutover(h) {
                self.bootstrap_target(cut_h, block.block.header.state_root, block.block.hash());
                self.cutover_height = Some(cut_h);
            }

            // Skip already-committed heights
            if self.committed.iter().any(|c| c.block.header.height == h) {
                continue;
            }

            // SAGE shadow validation
            if is_legacy && h >= self.controller.schedule.h_d && self.cutover_height.is_none() {
                let ctx = ReadinessContext {
                    chain_id: self.state.platform.chain_id.clone(),
                    epoch: self.state.platform.epoch,
                    config_id: self.state.platform.config_id,
                    target_engine: self.target.engine_id(),
                };
                let verdict = self.target.shadow_validate(&block.block, &self.state);
                let record = ShadowRecord {
                    height: h,
                    validator: self.id,
                    verdict_root: verdict.recomputed_root,
                    canonical_root: block.block.header.state_root,
                    valid: verdict.valid,
                };
                if let Some(cut_h) = self.controller.observe_shadow(record, ctx) {
                    match self.cutover_quorum {
                        // Legacy local-switch path (single-process LocalRuntime,
                        // simulator): switch the moment THIS node is ready.
                        None => {
                            self.cutover_height = Some(cut_h);
                            self.bootstrap_target(
                                cut_h,
                                block.block.header.state_root,
                                block.block.hash(),
                            );
                        }
                        // Quorum-gated path (multi-process testbed): do NOT
                        // switch on local readiness. Record the boundary this
                        // node attests to and count itself as one attester; the
                        // actual switch waits until n-f distinct validators
                        // attest the SAME boundary (checked in the run loop).
                        // Under a partition, a minority side can never reach n-f,
                        // so it never switches — that is the enforced safety.
                        Some(_) => {
                            let bhash = block.block.hash();
                            self.pending_cutover =
                                Some((cut_h, bhash, block.block.header.state_root, bhash));
                            let target_engine = self.target.engine_id();
                            let signature = self.sign_cutover_attestation(
                                cut_h,
                                bhash,
                                block.block.header.state_root,
                                target_engine,
                            )?;
                            let authenticated_share = self.verify_cutover_attestation(
                                self.id,
                                cut_h,
                                bhash,
                                block.block.header.state_root,
                                target_engine,
                                &signature,
                            )?;
                            self.cutover_attesters
                                .entry(CutoverKey {
                                    height: cut_h,
                                    boundary_block: bhash,
                                    boundary_root: block.block.header.state_root,
                                    target_engine,
                                })
                                .or_default()
                                .insert(self.id, authenticated_share);
                        }
                    }
                }
            }

            self.state = self.state.apply_block(&block.block)?;
            self.persist_finalized(block)?;
        }
        self.committed.extend(finalized.clone());
        Ok((finalized, pacemaker_msgs))
    }

    /// Quorum-gated cutover driver, called once per loop tick by the
    /// multi-process runtime. If this node has a pending (locally-ready) cutover
    /// boundary and the quorum gate is active, it (1) returns its own cutover
    /// attestation to broadcast, and (2) switches engines iff n-f DISTINCT
    /// validators have attested the SAME boundary. Returns any attestation
    /// message to broadcast this tick. No-op when the quorum gate is disabled
    /// (`cutover_quorum == None`) or there is no pending boundary.
    fn drive_quorum_cutover(&mut self) -> NodeResult<Vec<MessageEnvelope>> {
        let Some(threshold) = self.cutover_quorum else {
            return Ok(Vec::new());
        };
        if self.cutover_height.is_some() {
            return Ok(Vec::new()); // already switched
        }
        let Some((cut_h, bhash, root, cert)) = self.pending_cutover else {
            return Ok(Vec::new());
        };

        // Switch iff the boundary has n-f distinct attesters. The decision
        // predicate is the pure `cutover_gate::gate_open`, which is the exact
        // function the TLA+/Rust conformance harness exercises (see
        // crates/sage-node/tests/formal_conformance.rs and formal/CORRESPONDENCE.md),
        // so the runtime and the model check share one gate implementation.
        let attesters = self
            .cutover_attesters
            .get(&CutoverKey {
                height: cut_h,
                boundary_block: bhash,
                boundary_root: root,
                target_engine: self.target.engine_id(),
            })
            .map(|s| s.len() as u64)
            .unwrap_or(0);
        if cutover_gate::gate_open(attesters, threshold) {
            #[cfg(feature = "real-crypto")]
            {
                use sage_consensus::QuorumThreshold;
                use sage_manifest::real_ed25519::RealEd25519Scheme;

                let key = CutoverKey {
                    height: cut_h,
                    boundary_block: bhash,
                    boundary_root: root,
                    target_engine: self.target.engine_id(),
                };
                let shares = self.cutover_attesters.get(&key).cloned().ok_or_else(|| {
                    NodeError::Consensus("cutover quorum opened without signature shares".into())
                })?;
                let cut_cert = sage_manifest::CutoverCertificate {
                    payload: self.cutover_payload(cut_h, bhash, root, key.target_engine),
                    shares,
                };
                let validators = ValidatorSet::equal_power(
                    self.state.platform.config_id,
                    self.state.platform.epoch,
                    self.committee.len() as u32,
                );
                let mut scheme = RealEd25519Scheme::new();
                for signer in &self.committee {
                    scheme.register_deterministic(*signer, self.attestation_key_seed);
                }
                cut_cert
                    .verify(
                        &validators,
                        QuorumThreshold {
                            required_power: threshold,
                        },
                        &scheme,
                    )
                    .map_err(|err| {
                        NodeError::Consensus(format!(
                            "invalid completed cutover certificate: {err}"
                        ))
                    })?;
                // Persist the verified authority decision before exposing target
                // authority. The next storage phase replaces this memory backend
                // with a transactional durable implementation.
                self.store.put_migration_decision(MigrationDecisionRecord {
                    version: 1,
                    required_power: threshold,
                    cut_cert,
                })?;
            }
            #[cfg(not(feature = "real-crypto"))]
            return Err(NodeError::Consensus(
                "quorum cutover cannot switch authority without real crypto".into(),
            ));

            self.cutover_height = Some(cut_h);
            self.bootstrap_target(cut_h, root, cert);
            self.pending_cutover = None;
            return Ok(Vec::new());
        }

        // Not yet quorate: (re)broadcast our attestation so peers can count us.
        let target_engine = self.target.engine_id();
        let signature = self.sign_cutover_attestation(cut_h, bhash, root, target_engine)?;
        Ok(vec![MessageEnvelope {
            from: self.id,
            to: None,
            chain_id: self.state.platform.chain_id.clone(),
            epoch: self.state.platform.epoch,
            config_id: self.state.platform.config_id,
            message: ConsensusMessage::Sage(SageMessage::CutoverAttestation {
                height: cut_h,
                boundary_block: bhash,
                boundary_root: root,
                target_engine,
                signature,
            }),
        }])
    }

    /// Expected proposer for a given height (round-robin).
    ///
    /// Phase-aware: in the PoA phase the proposer rotates by height. In the
    /// HotStuff phase the engine validates proposals against its VIEW-keyed
    /// leader (`leader_for(view) = view % n`), and the pacemaker bumps the view
    /// on every failed round. If the runtime kept selecting by height, the
    /// height-proposer would not match the engine's view-leader after any view
    /// advance, so the selected node's own engine refuses to propose, no block
    /// is produced, and the view runs away (observed: view 101..175 with nobody
    /// proposing at n>=5). Selecting by the target engine's current view keeps
    /// the runtime's proposer aligned with the engine's leader.
    fn proposer_for(&self, height: Height, n: u32) -> ValidatorId {
        let key = if self.authoritative_engine_at(height).kind == EngineKind::HotStuff {
            self.target.current_view().get()
        } else {
            height.get()
        };
        ValidatorId::new((key % n as u64) as u32)
    }

    /// Authoritative engine for a specific height (used by the phase-aware
    /// proposer selection above).
    fn authoritative_engine_at(&self, height: Height) -> EngineId {
        match self.strategy {
            StrategyKind::Sage | StrategyKind::CoxFaithful => {
                if self.cutover_height.map(|h| height >= h).unwrap_or(false) {
                    self.target.engine_id()
                } else {
                    self.legacy.engine_id()
                }
            }
            StrategyKind::HardFork | StrategyKind::SageBlind | StrategyKind::CoxStyle => {
                if height >= self.controller.schedule.h_c {
                    self.target.engine_id()
                } else {
                    self.legacy.engine_id()
                }
            }
            StrategyKind::StopTheWorld => {
                if height >= self.controller.schedule.h_r {
                    self.target.engine_id()
                } else {
                    self.legacy.engine_id()
                }
            }
            StrategyKind::ReconfigOnly => self.legacy.engine_id(),
        }
    }
}

// ---------------------------------------------------------------------------
// LocalRuntime
// ---------------------------------------------------------------------------

/// Runs N validator nodes in the same process.
pub struct LocalRuntime {
    transport: Arc<Mutex<InMemoryTransport>>,
    nodes: Vec<NodeValidator>,
    config: NodeConfig,
}

/// Build the canonical genesis chain state shared by every validator.
/// Factored out so both `LocalRuntime` (one process, many nodes) and the
/// multi-process `ProcessValidator` (one process, one node) construct
/// byte-identical initial state.
fn genesis_state(cfg: &NodeConfig) -> ChainState {
    ChainState {
        execution: ExecutionState::new_with_accounts(10, 1000),
        platform: PlatformState {
            chain_id: cfg.chain_id.clone(),
            epoch: cfg.epoch,
            config_id: cfg.config_id,
            migration: None,
        },
        consensus: ConsensusStateEnvelope {
            engine: EngineId::new(EngineKind::Poa, sage_core::EngineGeneration::new(1)),
            opaque: vec![],
        },
    }
}

/// Construct a single `NodeValidator` for `id` under `cfg`. This is the shared
/// builder used by both the single-process `LocalRuntime` and the
/// multi-process `ProcessValidator`, guaranteeing identical engine/controller
/// wiring across both topologies.
fn build_validator(id: ValidatorId, cfg: &NodeConfig) -> NodeValidator {
    let validators = ValidatorSet::equal_power(cfg.config_id, cfg.epoch, cfg.n);
    let engine_id = EngineId::new(EngineKind::Poa, sage_core::EngineGeneration::new(1));
    let target_engine_id = EngineId::new(EngineKind::HotStuff, sage_core::EngineGeneration::new(1));
    let state = genesis_state(cfg);
    let legacy = PoaEngine::new(id, engine_id, validators.clone(), Hash32::ZERO);
    let target = HotStuffEngine::new(id, target_engine_id, validators, cfg.f as u64);
    let controller = SageController::new(cfg.migration);
    NodeValidator::new(id, cfg, state, legacy, target, controller)
}

impl LocalRuntime {
    pub fn new(cfg: NodeConfig) -> NodeResult<Self> {
        let ids: Vec<ValidatorId> = cfg.validator_ids();
        let transport = Arc::new(Mutex::new(InMemoryTransport::new(ids.clone())));

        let nodes: Vec<NodeValidator> = ids
            .into_iter()
            .map(|id| build_validator(id, &cfg))
            .collect();

        Ok(Self {
            transport,
            nodes,
            config: cfg,
        })
    }

    pub fn restore_node_from_store(
        &mut self,
        index: usize,
        store: MemoryBackend,
    ) -> NodeResult<()> {
        let Some(node) = self.nodes.get_mut(index) else {
            return Err(NodeError::Controller(format!("unknown node index {index}")));
        };
        node.restore_from_store(store)
    }

    /// Run the testnet until `max_height` is reached or the timeout expires.
    pub fn run(&mut self) -> NodeResult<NodeMetrics> {
        let start = Instant::now();
        let timeout = Duration::from_secs(self.config.max_runtime_secs);
        let mut metrics = NodeMetrics::default();
        let mut last_proposal_height = Height::new(0);
        let _proposal_interval = Duration::from_millis(self.config.proposal_interval_ms);
        let view_timeout_ms = self.config.view_timeout_ms;

        loop {
            // Safety valve
            if start.elapsed() > timeout {
                break;
            }

            let top_height = self
                .nodes
                .iter()
                .flat_map(|v| v.committed.last().map(|b| b.block.header.height))
                .max()
                .unwrap_or(Height::new(0));
            if top_height >= self.config.max_height {
                metrics.migration_success = self.nodes.iter().all(|v| v.cutover_height.is_some());
                break;
            }

            // ── Deliver queued messages with transport locked ──
            {
                let mut tport = self
                    .transport
                    .lock()
                    .map_err(|e| NodeError::Lock(format!("transport mutex poisoned: {e}")))?;
                for i in 0..self.nodes.len() {
                    let id = self.nodes[i].id;
                    let msgs: Vec<MessageEnvelope> = tport.recv(id)?;
                    if msgs.is_empty() {
                        continue;
                    }
                    let mut outbound = Vec::new();
                    for msg in msgs {
                        let mut replies = self.nodes[i].handle_message(msg)?;
                        outbound.append(&mut replies);
                    }

                    // Try to finalize after delivering messages
                    let (_finalized, pacemaker_msgs) =
                        self.nodes[i].try_finalize(view_timeout_ms)?;
                    outbound.extend(pacemaker_msgs);

                    for msg in outbound {
                        let _ = tport.broadcast(self.nodes[i].id, msg);
                    }
                }
            }

            // ── Count finalized blocks ──
            for node in &self.nodes {
                for b in &node.committed {
                    let h = b.block.header.height.get();
                    if h > metrics.max_finalized_height {
                        metrics.max_finalized_height = h;
                    }
                }
            }
            metrics.finalized_blocks = metrics.max_finalized_height;

            // ── Propose new blocks ──
            let time_since_proposal = (start.elapsed().as_millis() as u64)
                .saturating_sub(last_proposal_height.get() * self.config.proposal_interval_ms);
            if time_since_proposal >= self.config.proposal_interval_ms {
                for i in 0..self.nodes.len() {
                    let node = &mut self.nodes[i];
                    let next = node.next_height();
                    if next > self.config.max_height {
                        continue;
                    }
                    let should_propose = node.id == node.proposer_for(next, self.config.n);

                    if should_propose {
                        if let Some(block) = node.propose(next)? {
                            let msg = if node.authoritative_engine().kind == EngineKind::Poa {
                                MessageEnvelope {
                                    from: node.id,
                                    to: None,
                                    chain_id: self.config.chain_id.clone(),
                                    epoch: self.config.epoch,
                                    config_id: self.config.config_id,
                                    message: ConsensusMessage::Poa(
                                        sage_consensus::PoaMessage::Proposal(block),
                                    ),
                                }
                            } else {
                                MessageEnvelope {
                                    from: node.id,
                                    to: None,
                                    chain_id: self.config.chain_id.clone(),
                                    epoch: self.config.epoch,
                                    config_id: self.config.config_id,
                                    message: ConsensusMessage::HotStuff(
                                        HotStuffMessage::Proposal {
                                            view: node.target.current_view(),
                                            block,
                                            justify: None,
                                        },
                                    ),
                                }
                            };

                            // Self-deliver: proposer handles own proposal (like the simulator
                            // schedules DeliverMessage events for all validators)
                            let replies = node.handle_message(msg.clone())?;
                            for reply in &replies {
                                // Self-deliver the reply (vote) too
                                let _ = node.handle_message(reply.clone())?;
                            }

                            // Broadcast proposal + replies to other validators
                            {
                                let mut tport = self.transport.lock().map_err(|e| {
                                    NodeError::Lock(format!("transport mutex poisoned: {e}"))
                                })?;
                                let _ = tport.broadcast(node.id, msg);
                                for reply in replies {
                                    let _ = tport.broadcast(node.id, reply);
                                }
                            }

                            // Try to finalize after self-delivery
                            let (_finalized, pacemaker_msgs) =
                                node.try_finalize(view_timeout_ms)?;
                            {
                                let mut tport = self.transport.lock().map_err(|e| {
                                    NodeError::Lock(format!("transport mutex poisoned: {e}"))
                                })?;
                                for pm in pacemaker_msgs {
                                    let _ = tport.broadcast(node.id, pm);
                                }
                            }

                            last_proposal_height = next;
                        }
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(10));
        }

        // Observed safety: run the fork detector over every validator's real
        // committed log. safety_violation is now a measured fact (two distinct
        // blocks finalized at the same height across validators), not a default.
        let logs: std::collections::BTreeMap<ValidatorId, Vec<FinalizedBlock>> = self
            .nodes
            .iter()
            .map(|n| (n.id, n.committed.clone()))
            .collect();
        let fork_report = fork_detector::detect_forks(&logs);
        metrics.safety_violation = fork_report.fork_observed();

        metrics.total_duration_secs = start.elapsed().as_secs_f64();
        Ok(metrics)
    }
}

// ---------------------------------------------------------------------------
// ProcessValidator — one validator, one process, transport-generic
// ---------------------------------------------------------------------------

pub use process_result::ProcessResult;

/// Drives exactly ONE validator over an arbitrary `Transport`. Unlike
/// `LocalRuntime` (which owns every node and self-delivers the proposer's
/// messages by reaching into peer state), this has access only to its own node
/// and a socket — so every message, including the leader's own proposal and
/// vote, must round-trip through the transport. This is the real distributed
/// topology required for the multi-process testbed (M1): the same
/// `NodeValidator` consensus logic, but with no shared memory between peers.
pub struct ProcessValidator<T: Transport> {
    node: NodeValidator,
    transport: T,
    config: NodeConfig,
    /// Optional HEIGHT-TRIGGERED partition: once this node's next height reaches
    /// `partition_at`, it restricts itself to `partition_keep` (its side of the
    /// split). The partition must engage AT the migration boundary, not from
    /// start: a from-start split stalls the PoA phase (Majority quorum n/2+1 is
    /// unreachable by a minority side), so no validator ever reaches cutover and
    /// the HotStuff fork window never opens. Engaging at h_c lets both sides
    /// finalize the PoA prefix, then enter the HotStuff phase partitioned where
    /// each side of 3 (n=6,f=1) independently reaches BFT quorum 2f+1=3.
    partition_keep: Option<BTreeSet<ValidatorId>>,
    partition_at: Option<Height>,
    partition_engaged: bool,
    /// If set, this node is Byzantine and EQUIVOCATES at the given height: as
    /// proposer it mints two distinct blocks for the same height and sends each
    /// to a disjoint half of its peers, the canonical equivocation fault. A
    /// correct BFT engine must still not fork (quorum intersection); this
    /// exercises the detector against a genuine Byzantine action rather than a
    /// partition.
    equivocate_at: Option<Height>,
    /// Commit timeline captured during the run: one entry per newly finalized
    /// height, recording (height, elapsed_micros_at_first_observation,
    /// cumulative_tx_count). Used to compute committed TPS and inter-commit
    /// finalization-gap percentiles (the empirical T_bdy liveness bound) without
    /// altering consensus. Populated only by the run loop; empty otherwise.
    commit_timeline: Vec<(u64, u64, u64)>,
}

impl<T: Transport> ProcessValidator<T> {
    /// Build the validator `id` under `cfg`, driven over `transport`. The node
    /// is constructed with the identical shared builder used by `LocalRuntime`,
    /// so single- and multi-process runs share byte-identical engine wiring.
    pub fn new(id: ValidatorId, cfg: NodeConfig, transport: T) -> Self {
        let node = build_validator(id, &cfg);
        // Coinbase is OFF by default. It makes block content proposer-dependent,
        // which is REQUIRED only for the partition fork-detection experiment
        // (two concurrent leaders must mint distinguishable blocks). In the
        // honest no-partition path a view change re-proposes a height under a
        // new leader; with a proposer-dependent coinbase that re-proposal has a
        // different state root, and a node that already committed the first
        // block rejects the second with a state-root mismatch. So enable it
        // explicitly via `with_coinbase()` only where concurrent distinct
        // proposals are the point (see `with_partition`).
        Self {
            node,
            transport,
            config: cfg,
            partition_keep: None,
            partition_at: None,
            partition_engaged: false,
            equivocate_at: None,
            commit_timeline: Vec::new(),
        }
    }

    /// Make this node Byzantine: when it is the proposer at `at_height` it mints
    /// two distinct blocks and sends each to a disjoint half of its peers
    /// (equivocation). Used to exercise the fork detector against a genuine
    /// Byzantine action; a correct quorum-intersecting engine must still not
    /// fork. Requires `with_coinbase` semantics to make the two blocks distinct.
    pub fn with_equivocation(mut self, at_height: Height) -> Self {
        self.equivocate_at = Some(at_height);
        self.node.coinbase_enabled = true;
        self
    }

    /// Enable the proposer-beneficiary (coinbase) transaction so two concurrent
    /// leaders at the same height mint DISTINCT blocks — required for the
    /// partition fork-detection experiment, where the detector must be able to
    /// see two different boundary blocks. Leave OFF for honest/no-fault runs.
    pub fn with_coinbase(mut self) -> Self {
        self.node.coinbase_enabled = true;
        self
    }

    /// Arm a height-triggered partition: when this node's next height reaches
    /// `at_height`, it restricts sends to `keep` (its own side). See the field
    /// docs for why the trigger must be at/after the cutover height.
    pub fn with_partition(mut self, keep: BTreeSet<ValidatorId>, at_height: Height) -> Self {
        self.partition_keep = Some(keep);
        self.partition_at = Some(at_height);
        self
    }

    /// Enable the QUORUM-GATED cutover with threshold `n-f` (the SAGE
    /// MigrationCutover quorum). With this set, a SAGE validator does NOT switch
    /// engines on local readiness; it broadcasts a cutover attestation and only
    /// switches once `n-f` distinct validators attest the same boundary. Under a
    /// partition a minority side never reaches `n-f`, so it never switches — the
    /// enforced safety. Only meaningful for the SAGE strategy; HardFork/blind
    /// ignore it and switch unconditionally at h_c (the broken control).
    pub fn with_cutover_quorum(mut self) -> Self {
        let threshold = (self.config.n - self.config.f) as u64;
        self.node.cutover_quorum = Some(threshold);
        self
    }

    /// Enable faithful Cox's checkpoint-quorum gate with threshold `2f+1` (the
    /// target BFT quorum, Cox's StableCheckpoint quorum), as opposed to SAGE's
    /// `n-f`. Same code path as `with_cutover_quorum`, only the threshold
    /// differs — the cleanest isolation of SAGE's contribution. With n=6, f=1
    /// this is 3, which EACH side of a 3/3 partition can reach (two disjoint
    /// 3-quorums fit in 6 once n>3f+1), so faithful Cox's own gate still admits
    /// a cross-boundary fork, whereas SAGE's n-f=5 does not.
    pub fn with_cox_checkpoint_quorum(mut self) -> Self {
        let threshold = (2 * self.config.f + 1) as u64;
        self.node.cutover_quorum = Some(threshold);
        self
    }

    /// Engage the height-triggered partition the moment this node's next height
    /// reaches the trigger. Must be called BOTH at loop-top AND immediately
    /// after finalize: finalizing the cutover-prefix block flips next_height to
    /// h_c within a single iteration, and if the partition is not re-checked
    /// before the propose step the new HotStuff leader broadcasts its h_c block
    /// to ALL peers (the partition has not engaged yet), so the other side rides
    /// that block instead of timing out and minting its own — no fork. Checking
    /// here closes that one-iteration leak.
    fn maybe_engage_partition(&mut self) {
        if self.partition_engaged {
            return;
        }
        if let (Some(keep), Some(at)) = (&self.partition_keep, self.partition_at) {
            if self.node.next_height() >= at {
                self.transport.partition_to(keep);
                self.partition_engaged = true;
            }
        }
    }

    /// Run until this node finalizes `max_height` or the timeout expires.
    pub fn run(&mut self) -> NodeResult<ProcessResult> {
        let start = Instant::now();
        let timeout = Duration::from_secs(self.config.max_runtime_secs);
        let view_timeout_ms = self.config.view_timeout_ms;
        let proposal_interval_ms = self.config.proposal_interval_ms;
        let n = self.config.n;
        let id = self.node.id;
        let mut last_proposal_height = Height::new(0);

        loop {
            if start.elapsed() > timeout {
                break;
            }
            if self.node.next_height().get() > self.config.max_height.get() {
                break;
            }

            // ── 0. Engage height-triggered partition at the migration boundary ──
            self.maybe_engage_partition();

            // ── 1. Deliver inbound messages, broadcast replies ──
            let inbound = self.transport.recv(id)?;
            let mut outbound = Vec::new();
            for msg in inbound {
                let mut replies = self.node.handle_message_tolerant(msg)?;
                outbound.append(&mut replies);
            }
            let (_f, pacemaker) = self.node.try_finalize(view_timeout_ms)?;
            // Self-deliver pacemaker (timeout/TC) messages so this node counts
            // its OWN timeout toward the 2f+1 timeout certificate, exactly as
            // the leader self-delivers its own proposal/vote below. Without
            // this, a partitioned side of 3 collects only its 2 peers' timeouts
            // (one short of threshold 3), never forms a TC, and so can never
            // rotate from view 0 to an in-partition leader — it freezes.
            for pm in &pacemaker {
                let _ = self.node.handle_message_tolerant(pm.clone())?;
            }
            outbound.extend(pacemaker);
            for msg in outbound {
                self.transport.broadcast(id, msg)?;
            }

            // Re-check the partition: finalizing above may have flipped
            // next_height to the cutover trigger within THIS iteration. Engage
            // now so the propose step below does not leak the first HotStuff
            // block across the (about-to-form) partition boundary.
            self.maybe_engage_partition();

            // ── 1b. Drive the quorum-gated cutover (SAGE safety enforcement) ──
            // If this node is locally ready to cut over, broadcast its cutover
            // attestation and switch engines iff n-f distinct validators have
            // attested the same boundary. A partitioned minority never reaches
            // n-f, so it never switches — no fork.
            for msg in self.node.drive_quorum_cutover()? {
                self.transport.broadcast(id, msg)?;
            }

            // ── 2. Propose if leader for the next height ──
            let next = self.node.next_height();
            if next.get() <= self.config.max_height.get() {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                let due = elapsed_ms
                    .saturating_sub(last_proposal_height.get() * proposal_interval_ms)
                    >= proposal_interval_ms;
                if due && id == self.node.proposer_for(next, n) {
                    let equivocating = self.equivocate_at == Some(next);
                    if equivocating {
                        // BYZANTINE EQUIVOCATION: mint two distinct blocks for the
                        // same height (variant 1 and variant 2 -> distinct coinbase
                        // -> distinct hashes) and send each to a disjoint half of
                        // the peers. A correct quorum-intersecting BFT engine must
                        // STILL not fork: no two n-f (here 2f+1) quorums can both
                        // form on conflicting blocks. This exercises the detector
                        // against a real Byzantine action rather than a partition.
                        let is_poa = self.node.authoritative_engine().kind == EngineKind::Poa;
                        let b1 = self.node.propose_variant(next, 1)?;
                        let b2 = self.node.propose_variant(next, 2)?;
                        if let (Some(b1), Some(b2)) = (b1, b2) {
                            let m1 = self.build_proposal(b1, is_poa);
                            let m2 = self.build_proposal(b2, is_poa);
                            // Self-deliver one variant (the node acts on its own b1).
                            let replies = self.node.handle_message_tolerant(m1.clone())?;
                            for reply in &replies {
                                let _ = self.node.handle_message_tolerant(reply.clone())?;
                            }
                            // Split peers into two disjoint halves by id parity.
                            let peers: Vec<ValidatorId> = self
                                .transport
                                .peers()
                                .into_iter()
                                .filter(|p| *p != id)
                                .collect();
                            for (idx, peer) in peers.iter().enumerate() {
                                let m = if idx % 2 == 0 { m1.clone() } else { m2.clone() };
                                self.transport.send(id, *peer, m)?;
                            }
                            for reply in replies {
                                self.transport.broadcast(id, reply)?;
                            }
                            last_proposal_height = next;
                        }
                    } else if let Some(block) = self.node.propose(next)? {
                        let is_poa = self.node.authoritative_engine().kind == EngineKind::Poa;
                        let msg = self.build_proposal(block, is_poa);
                        // Self-deliver own proposal + own vote (no shared memory:
                        // the leader must process its own message just like a peer).
                        let replies = self.node.handle_message_tolerant(msg.clone())?;
                        for reply in &replies {
                            let _ = self.node.handle_message_tolerant(reply.clone())?;
                        }
                        self.transport.broadcast(id, msg)?;
                        for reply in replies {
                            self.transport.broadcast(id, reply)?;
                        }
                        let (_f2, pacemaker2) = self.node.try_finalize(view_timeout_ms)?;
                        for pm in &pacemaker2 {
                            let _ = self.node.handle_message_tolerant(pm.clone())?;
                        }
                        for pm in pacemaker2 {
                            self.transport.broadcast(id, pm)?;
                        }
                        last_proposal_height = next;
                    }
                }
            }

            // ── Capture commit timeline for TPS / inter-commit-gap metrics ──
            // Record any heights finalized since the last iteration with the
            // elapsed wall-clock time and cumulative tx count. This is a passive
            // observation; it never influences consensus.
            let observed = self.commit_timeline.len();
            if self.node.committed.len() > observed {
                let now_micros = start.elapsed().as_micros() as u64;
                let mut cum_txs: u64 = self.commit_timeline.last().map(|(_, _, t)| *t).unwrap_or(0);
                for b in self.node.committed.iter().skip(observed) {
                    cum_txs = cum_txs.saturating_add(b.block.txs.len() as u64);
                    self.commit_timeline
                        .push((b.block.header.height.get(), now_micros, cum_txs));
                }
            }

            std::thread::sleep(Duration::from_millis(self.config.pacemaker_tick_ms.min(10)));
        }

        Ok(self.build_result(start.elapsed().as_secs_f64()))
    }

    fn build_proposal(&self, block: Block, is_poa: bool) -> MessageEnvelope {
        let message = if is_poa {
            ConsensusMessage::Poa(sage_consensus::PoaMessage::Proposal(block))
        } else {
            ConsensusMessage::HotStuff(HotStuffMessage::Proposal {
                view: self.node.target.current_view(),
                block,
                justify: None,
            })
        };
        MessageEnvelope {
            from: self.node.id,
            to: None,
            chain_id: self.config.chain_id.clone(),
            epoch: self.config.epoch,
            config_id: self.config.config_id,
            message,
        }
    }

    fn build_result(&self, secs: f64) -> ProcessResult {
        let committed: Vec<(u64, String)> = self
            .node
            .committed
            .iter()
            .map(|b| (b.block.header.height.get(), b.block.hash().to_string()))
            .collect();
        let max_h = committed.iter().map(|(h, _)| *h).max().unwrap_or(0);
        let committed_detail: Vec<(u64, String, String)> = self
            .node
            .committed
            .iter()
            .map(|b| {
                (
                    b.block.header.height.get(),
                    b.block.header.state_root.to_string(),
                    format!("{:?}", b.block.header.engine_id.kind),
                )
            })
            .collect();
        // ── Throughput and inter-commit finalization-gap metrics ──
        // Computed from the passively captured commit timeline (height,
        // elapsed_micros, cumulative_txs). TPS is committed transactions over
        // wall-clock; the inter-commit gaps are the empirical realization of
        // the T_bdy liveness bound (paper Thm 3), where the max gap includes
        // the one-time cutover handoff and the tail should not diverge.
        let total_txs = self.commit_timeline.last().map(|(_, _, t)| *t).unwrap_or(0);
        let committed_tps = if secs > 0.0 {
            total_txs as f64 / secs
        } else {
            0.0
        };
        let mut gaps: Vec<u64> = Vec::new();
        let mut cutover_gap_micros = 0u64;
        let h_c = self.config.migration.h_c.get();
        for w in self.commit_timeline.windows(2) {
            let (h_prev, t_prev, _) = w[0];
            (_, _) = (h_prev, t_prev);
            let (h_cur, t_cur, _) = w[1];
            let gap = t_cur.saturating_sub(w[0].1);
            gaps.push(gap);
            // The gap whose upper height is the cutover height is the observed
            // handoff perturbation.
            if h_cur == h_c {
                cutover_gap_micros = gap;
            }
        }
        gaps.sort_unstable();
        let pct = |p: f64| -> u64 {
            if gaps.is_empty() {
                return 0;
            }
            let idx = ((gaps.len() as f64 - 1.0) * p).round() as usize;
            gaps[idx.min(gaps.len() - 1)]
        };
        let inter_commit_p50_micros = pct(0.50);
        let inter_commit_p95_micros = pct(0.95);
        let inter_commit_p99_micros = pct(0.99);
        let inter_commit_max_micros = gaps.iter().copied().max().unwrap_or(0);

        let (cert_sig, cert_hash) = self.sign_cutover_cert();
        let replay_context_root_hash = self.replay_context_root_hash();
        let replay_context_root = replay_context_root_hash.map(|root| root.to_string());
        let manifest_payload_hash = self
            .manifest_payload_hash(replay_context_root_hash)
            .map(|hash| hash.to_string());
        ProcessResult {
            validator: self.node.id.get(),
            migration_success: self.node.cutover_height.is_some()
                && max_h >= self.config.max_height.get(),
            cutover_height: self.node.cutover_height.map(|h| h.get()),
            max_finalized_height: max_h,
            total_duration_secs: secs,
            committed,
            cutover_cert: cert_sig,
            cutover_payload_hash: cert_hash,
            seed: self.config.workload_seed,
            messages_sent: self.transport.sent_count(),
            committed_detail,
            replay_context_root,
            manifest_payload_hash,
            total_txs,
            committed_tps,
            inter_commit_p50_micros,
            inter_commit_p95_micros,
            inter_commit_p99_micros,
            inter_commit_max_micros,
            cutover_gap_micros,
        }
    }

    fn replay_context_root_hash(&self) -> Option<Hash32> {
        // Bind the cutover replay anchor itself; later reversible-window blocks
        // may legitimately differ across validators that make uneven progress.
        let blocks: Vec<_> = self
            .node
            .committed
            .iter()
            .filter(|b| {
                let h = b.block.header.height;
                h == self.config.migration.h_c
            })
            .map(|b| ReplayBlockContext {
                chain_id: b.block.header.chain_id.clone(),
                height: b.block.header.height,
                // The current testbed executor has no timestamp/basefee/randomness
                // source. Use explicit defaults so the generated root is stable
                // and auditable until a production VM supplies real values.
                timestamp_micros: b.block.header.height.get().saturating_mul(1_000_000),
                beneficiary: Hash32::ZERO,
                base_fee: 0,
                randomness: Hash32::ZERO,
                oracle_snapshot_root: None,
            })
            .collect();
        if blocks.is_empty() {
            None
        } else {
            Some(ReplayContext::new(blocks).root())
        }
    }

    fn manifest_payload_hash(&self, replay_context_root: Option<Hash32>) -> Option<Hash32> {
        use sage_manifest::certificate::{CertificateKind, CertificatePayload};
        use sage_manifest::{
            Certificate, ManifestBuilder, SignatureEnvelope, SimulatedSignatureScheme,
        };
        use std::collections::BTreeSet;

        let cut_h = self.node.cutover_height?;
        let block = self
            .node
            .committed
            .iter()
            .find(|b| b.block.header.height == cut_h)?;
        let payload = CertificatePayload {
            chain_id: self.config.chain_id.clone(),
            epoch: self.config.epoch,
            config_id: self.config.config_id,
            kind: CertificateKind::Cutover,
            height: cut_h,
            root: block.block.header.state_root,
            block_hash: Some(block.block.hash()),
            engine_id: self.node.target.engine_id(),
        };
        let payload_hash = sage_core::crypto::hash_canonical(
            sage_core::crypto::HashDomain::CertificateV1,
            &payload,
        );
        let cut_cert = Certificate {
            payload,
            signers: BTreeSet::from([self.node.id]),
            signature: SignatureEnvelope::Simulated { payload_hash },
        };
        let manifest = ManifestBuilder::new(SimulatedSignatureScheme, self.node.id)
            .build_with_replay_context_root(
                self.config.chain_id.clone(),
                self.config.epoch,
                self.config.config_id,
                cut_h,
                block.block.header.parent_hash,
                block.block.header.state_root,
                self.node.legacy.engine_id(),
                self.node.target.engine_id(),
                Hash32::ZERO,
                replay_context_root,
                cut_cert,
            )
            .ok()?;
        Some(manifest.signing_payload())
    }

    /// Sign this validator's view of the cutover certificate with its real
    /// ed25519 key (testbed M1). Returns (signature_hex, payload_hash_hex) so
    /// the orchestrator can collect 2f+1 distinct valid signatures and verify a
    /// genuine threshold CutCert — the cryptographic witness that the migration
    /// was sealed by a quorum, not a flag flip. Without the `real-crypto`
    /// feature this is (None, None) (simulated scheme path).
    #[cfg(feature = "real-crypto")]
    fn sign_cutover_cert(&self) -> (Option<String>, Option<String>) {
        use sage_manifest::certificate::{CertificateKind, CertificatePayload};
        use sage_manifest::real_ed25519::RealEd25519Scheme;
        use sage_manifest::{SignatureEnvelope, SignatureScheme};

        let Some(cut_h) = self.node.cutover_height else {
            return (None, None);
        };
        // The cutover certificate binds the sealed block at the cutover height.
        let Some(block) = self
            .node
            .committed
            .iter()
            .find(|b| b.block.header.height == cut_h)
        else {
            return (None, None);
        };
        let payload = CertificatePayload {
            chain_id: self.config.chain_id.clone(),
            epoch: self.config.epoch,
            config_id: self.config.config_id,
            kind: CertificateKind::Cutover,
            height: cut_h,
            root: block.block.header.state_root,
            block_hash: Some(block.block.hash()),
            engine_id: self.node.target.engine_id(),
        };
        let hash = sage_core::crypto::hash_canonical(
            sage_core::crypto::HashDomain::CertificateV1,
            &payload,
        );
        let mut scheme = RealEd25519Scheme::new();
        scheme.register_deterministic(self.node.id, self.config.workload_seed);
        match scheme.sign(self.node.id, hash) {
            Ok(SignatureEnvelope::Ed25519 {
                signature_bytes, ..
            }) => (Some(hex_encode(&signature_bytes)), Some(hash.to_string())),
            _ => (None, None),
        }
    }

    #[cfg(not(feature = "real-crypto"))]
    fn sign_cutover_cert(&self) -> (Option<String>, Option<String>) {
        (None, None)
    }
}

#[cfg(feature = "real-crypto")]
fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
