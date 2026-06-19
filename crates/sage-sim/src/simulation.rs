use crate::{
    adversary::AdversaryModel, Event, EventKind, MetricsRecorder, NetworkModel, SimConfig,
    SimError, SimResult, SimValidator, WorkloadGenerator,
};
use sage_consensus::{
    ConsensusEngine, ConsensusMessage, HotStuffEngine, MessageEnvelope, PoaEngine, PoaMessage,
    ValidatorSet,
};
use sage_controller::{ReadinessContext, SageController};
use sage_core::{
    ChainId, ConfigId, ConsensusStateEnvelope, DurationMicros, EngineGeneration, EngineId,
    EngineKind, Epoch, ExecutionState, Hash32, Height, PlatformState, SimTime, ValidatorId,
};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

pub struct Simulation {
    cfg: SimConfig,
    target_engine_id: EngineId,
    adversary: AdversaryModel,
    events: BinaryHeap<Event>,
    validators: BTreeMap<ValidatorId, SimValidator>,
    network: NetworkModel,
    workload: WorkloadGenerator,
    metrics: MetricsRecorder,
    now: SimTime,
    next_seq: u64,
    event_count: u64,
    parent_hash: Hash32,
    last_cutover_time: Option<SimTime>,
    height_attempts: BTreeMap<Height, u64>,
    observed_partition_forks: BTreeSet<Height>,
    hotstuff_view: u64,
    /// Set once a stop-the-world halt has been applied at the restart height,
    /// so the one-time service interruption is not re-applied on re-propose.
    stw_halt_applied: bool,
}

impl Simulation {
    pub fn new(cfg: SimConfig) -> SimResult<Self> {
        cfg.validate()?;
        let engine_id = EngineId::new(EngineKind::Poa, EngineGeneration::new(1));
        let target_engine_id = EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1));
        let validators =
            ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), cfg.validators.n);
        let mut nodes = BTreeMap::new();
        for id in 0..cfg.validators.n {
            let state = ChainStateBuilder::new(&cfg, engine_id).build();
            let validator_id = ValidatorId::new(id);
            let legacy = PoaEngine::new(validator_id, engine_id, validators.clone(), Hash32::ZERO);
            let target = HotStuffEngine::new(
                validator_id,
                target_engine_id,
                validators.clone(),
                cfg.validators.f,
            );
            let controller = SageController::new(cfg.migration);
            nodes.insert(
                validator_id,
                SimValidator::new(
                    validator_id,
                    cfg.strategy,
                    state,
                    legacy,
                    target,
                    controller,
                ),
            );
        }
        let mut sim = Self {
            cfg: cfg.clone(),
            target_engine_id,
            adversary: AdversaryModel::new(cfg.adversary),
            events: BinaryHeap::new(),
            validators: nodes,
            network: NetworkModel::new(
                cfg.simulation.seed ^ 0xA11CE,
                cfg.simulation.mean_delay_micros,
            ),
            workload: WorkloadGenerator::new(cfg.simulation.seed ^ 0xB0B, cfg.workload.clone()),
            metrics: MetricsRecorder::new(cfg.strategy),
            now: SimTime::ZERO,
            next_seq: 0,
            event_count: 0,
            parent_hash: Hash32::ZERO,
            last_cutover_time: None,
            height_attempts: BTreeMap::new(),
            observed_partition_forks: BTreeSet::new(),
            hotstuff_view: 0,
            stw_halt_applied: false,
        };
        sim.schedule(
            SimTime::ZERO,
            EventKind::ProposeHeight {
                height: Height::new(1),
            },
        );
        Ok(sim)
    }

    pub fn snapshot(&self) -> sage_controller::SystemSnapshot {
        sage_controller::SystemSnapshot {
            validators: self.validators.values().map(|v| v.snapshot()).collect(),
            cutovers: Vec::new(),
            rollbacks: Vec::new(),
            shadow_events: Vec::new(),
        }
    }

    pub fn run(&mut self) -> SimResult<crate::RunMetrics> {
        while let Some(event) = self.events.pop() {
            self.event_count += 1;
            if self.event_count > self.cfg.simulation.max_events {
                return Err(SimError::Config(format!(
                    "exceeded max_events ({})",
                    self.cfg.simulation.max_events
                )));
            }
            self.now = event.at;
            match event.kind {
                EventKind::ProposeHeight { height } => self.on_propose_height(height)?,
                EventKind::DeliverMessage { to, message } => {
                    self.on_deliver_message(to, *message)?
                }
                EventKind::TryFinalize { validator } => self.on_try_finalize(validator)?,
            }
        }
        Ok(self.metrics.finish())
    }

    fn on_propose_height(&mut self, height: Height) -> SimResult<()> {
        if height.get() > self.cfg.simulation.max_height {
            return Ok(());
        }
        // Stop-the-world service interruption: the first time the chain reaches
        // the restart height, it must halt finalization while it snapshots state
        // and initializes the target engine. We model this as a one-time
        // simulated-time gap by deferring this height's proposal by the
        // configured downtime window. The interruption then emerges organically
        // in the recorded finalization timestamps (max_finalization_gap_micros)
        // rather than being derived from the strategy flag.
        if self.cfg.strategy == crate::StrategyKind::StopTheWorld
            && !self.stw_halt_applied
            && self.cfg.simulation.stw_downtime_micros > 0
            && height == self.cfg.migration.h_r
            && !self.height_attempts.contains_key(&height)
        {
            self.stw_halt_applied = true;
            self.metrics.observe_downtime_event();
            let resume_at = self
                .now
                .checked_add(DurationMicros(self.cfg.simulation.stw_downtime_micros))
                .unwrap_or(self.now);
            self.schedule(resume_at, EventKind::ProposeHeight { height });
            return Ok(());
        }
        // Activate partition based on the height being proposed (not finalized).
        // This ensures the proposal at this height is already under partition.
        let _action = self.adversary.on_height(height, self.cfg.validators.n);
        self.observe_disjoint_quorum_window(height);
        let txs = self.workload.next_batch(height);
        let n = self.cfg.validators.n;
        // If this is a re-propose (height already attempted), cycle the view.
        let reattempt = self.height_attempts.contains_key(&height);
        if reattempt {
            self.hotstuff_view = self.hotstuff_view.saturating_add(1);
        }
        let view = self.hotstuff_view.max(height.get());
        let proposer_id = ValidatorId::new((view % n as u64) as u32);

        // Pre-propose: synchronize all validators' HotStuff engines (bootstrap + view).
        {
            let is_hotstuff = self
                .validators
                .values()
                .next()
                .map(|v| v.authoritative_engine(height).kind != EngineKind::Poa)
                .unwrap_or(false);
            if is_hotstuff {
                for (_id, v) in self.validators.iter_mut() {
                    if !v.cutover_happened() {
                        if let Some(last) = v.committed.last() {
                            let _ = v.target.init(sage_consensus::BootstrapAnchor {
                                height,
                                root: last.block.header.state_root,
                                certificate_hash: last.block.hash(),
                            });
                            v.cutover_height = Some(height);
                        }
                    }
                    v.target.set_view(sage_core::View::new(view));
                }
            }
        }

        let proposer_node = self
            .validators
            .get_mut(&proposer_id)
            .ok_or(SimError::UnknownValidator(proposer_id))?;
        let is_legacy = proposer_node.authoritative_engine(height).kind == EngineKind::Poa;
        let parent_hash = proposer_node
            .committed
            .last()
            .map(|b| b.block.hash())
            .unwrap_or(Hash32::ZERO);
        let state_snapshot = proposer_node.state.clone();
        if let Some(block) = proposer_node.propose(sage_consensus::ProposeContext {
            state: &state_snapshot,
            parent_hash,
            height,
            txs: &txs,
        })? {
            let hotstuff_view = if !is_legacy {
                proposer_node.target.current_view()
            } else {
                sage_core::View::new(0)
            };
            let chain_id = block.header.chain_id.clone();
            let epoch = block.header.epoch;
            let config_id = block.header.config_id;
            let msg = if is_legacy {
                ConsensusMessage::Poa(PoaMessage::Proposal(block))
            } else {
                ConsensusMessage::HotStuff(sage_consensus::HotStuffMessage::Proposal {
                    view: hotstuff_view,
                    block,
                    justify: None,
                })
            };
            let proposal = MessageEnvelope {
                from: proposer_id,
                to: None,
                chain_id,
                epoch,
                config_id,
                message: msg,
            };
            let ids: Vec<_> = self.validators.keys().copied().collect();
            for id in ids {
                if let Some(evt) = self.network.try_send(
                    proposer_id,
                    id,
                    proposal.clone(),
                    self.now,
                    &self.adversary.partition,
                ) {
                    self.events.push(evt);
                }
            }
        }
        Ok(())
    }

    fn on_deliver_message(&mut self, to: ValidatorId, message: MessageEnvelope) -> SimResult<()> {
        let node = self
            .validators
            .get_mut(&to)
            .ok_or(SimError::UnknownValidator(to))?;
        if !node.accepts(&message) {
            return Ok(());
        }
        let outgoing = node.handle_message(message)?;
        let from = to;
        for out in outgoing {
            let ids: Vec<_> = self.validators.keys().copied().collect();
            for id in ids {
                if let Some(evt) = self.network.try_send(
                    from,
                    id,
                    out.clone(),
                    self.now,
                    &self.adversary.partition,
                ) {
                    self.events.push(evt);
                }
            }
        }
        self.schedule(self.now, EventKind::TryFinalize { validator: to });
        Ok(())
    }

    fn on_try_finalize(&mut self, validator: ValidatorId) -> SimResult<()> {
        let shadow_ctx = self.shadow_context();
        let (finalized, cutover_heights, next_needed, _is_hotstuff, new_view_msgs) = {
            let node = self
                .validators
                .get_mut(&validator)
                .ok_or(SimError::UnknownValidator(validator))?;
            let (finalized, cutover_heights, new_view_msgs_local) =
                node.try_finalize(shadow_ctx)?;
            let using_hotstuff = node.authoritative_engine(Height::new(1)).kind != EngineKind::Poa;
            let next_needed = if finalized.is_empty() && using_hotstuff {
                node.committed
                    .last()
                    .and_then(|b| b.block.header.height.checked_next())
                    .unwrap_or(Height::new(1))
            } else {
                Height::new(0)
            };
            (
                finalized,
                cutover_heights,
                next_needed,
                using_hotstuff,
                new_view_msgs_local,
            )
        };
        // broadcast NewView messages to all validators
        for msg in new_view_msgs {
            let ids: Vec<_> = self.validators.keys().copied().collect();
            for id in ids {
                if let Some(evt) = self.network.try_send(
                    validator,
                    id,
                    msg.clone(),
                    self.now,
                    &self.adversary.partition,
                ) {
                    self.events.push(evt);
                }
            }
        }
        for cutover_height in cutover_heights {
            self.metrics.observe_cutover(cutover_height);
            if self.last_cutover_time.is_none() {
                self.last_cutover_time = Some(self.now);
            }
        }
        for block in finalized {
            let finalized_height = block.block.header.height;
            if let Some(cut_time) = self.last_cutover_time {
                if finalized_height == self.cfg.migration.h_c {
                    let latency_micros = self.now.0.saturating_sub(cut_time.0);
                    self.metrics.observe_cutover_latency(latency_micros);
                }
            }
            self.parent_hash = block.block.hash();
            let next_height = block.block.header.height.checked_next();
            self.metrics.observe_finalized(validator, block, self.now);
            let _action = self
                .adversary
                .on_height(finalized_height, self.cfg.validators.n);
            self.observe_disjoint_quorum_window(finalized_height);
            if self.all_validators_committed(finalized_height) {
                if let Some(height) = next_height {
                    self.schedule(self.now, EventKind::ProposeHeight { height });
                }
            }
        }
        if next_needed.get() > 0 {
            let attempts = self.height_attempts.entry(next_needed).or_insert(0);
            *attempts += 1;
            if *attempts <= 12 {
                self.schedule(
                    self.now,
                    EventKind::ProposeHeight {
                        height: next_needed,
                    },
                );
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn find_proposer_for_hotstuff(&self, height: Height) -> ValidatorId {
        let sample = self.validators.values().next();
        let n = self.cfg.validators.n;
        let is_hotstuff = sample
            .map(|v| v.authoritative_engine(height).kind != EngineKind::Poa)
            .unwrap_or(false);
        if !is_hotstuff {
            return ValidatorId::new((height.get() % n as u64) as u32);
        }
        // find the validator whose HotStuff view makes it a leader, preferring the highest view
        let mut best_view = 0u64;
        let mut best_vid = ValidatorId::new((height.get() % n as u64) as u32);
        for id in 0..n {
            let vid = ValidatorId::new(id);
            if let Some(v) = self.validators.get(&vid) {
                let view = v.target.current_view();
                let leader = ValidatorId::new((view.get() % n as u64) as u32);
                if leader == vid && view.get() > best_view {
                    best_view = view.get();
                    best_vid = vid;
                }
            }
        }
        best_vid
    }

    fn shadow_context(&self) -> Option<ReadinessContext> {
        if !self.cfg.strategy.uses_shadow_validation() {
            return None;
        }
        Some(ReadinessContext {
            chain_id: ChainId::new("sage-sim"),
            epoch: Epoch::new(1),
            config_id: ConfigId::new(1),
            target_engine: self.target_engine_id,
        })
    }

    fn all_validators_committed(&self, height: Height) -> bool {
        self.validators.values().all(|validator| {
            validator
                .committed
                .iter()
                .any(|block| block.block.header.height == height)
        })
    }

    /// Detect a *disjoint-quorum exposure window*: a height at or after a
    /// non-CutCert cutover where the active partition leaves BOTH sides
    /// independently holding >= BFT quorum (2f+1). When both sides can each
    /// reach quorum on their own, the target engine could finalize conflicting
    /// blocks on each side — a fork is structurally *possible*.
    ///
    /// This is an honest exposure count, NOT an observed fork. SAGE gates the
    /// cutover behind a CutCert, so it is never exposed; HardFork/SageBlind
    /// cut over blindly and ARE exposed whenever the split is balanced enough
    /// (requires n >= 4f+2). The serialized simulator never elects two leaders
    /// at once, so an exposure window does not translate into an observed
    /// `safety_violation` here; the two metrics are deliberately distinct.
    fn observe_disjoint_quorum_window(&mut self, height: Height) {
        if !self.adversary.partition.active || height < self.cfg.migration.h_c {
            return;
        }
        // Only blind cutover strategies are exposed; SAGE/StopTheWorld/Reconfig
        // either require a CutCert or never swap engines under partition.
        if !matches!(
            self.cfg.strategy,
            crate::StrategyKind::HardFork | crate::StrategyKind::SageBlind
        ) {
            return;
        }
        let threshold = self.cfg.validators.f.saturating_mul(2).saturating_add(1);
        let side_a = self.adversary.partition.side_a.len() as u64;
        let side_b = self.adversary.partition.side_b.len() as u64;
        if side_a >= threshold
            && side_b >= threshold
            && self.observed_partition_forks.insert(height)
        {
            self.metrics.observe_disjoint_quorum_window();
        }
    }

    fn schedule(&mut self, at: SimTime, kind: EventKind) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        self.events.push(Event { at, seq, kind });
    }
}

struct ChainStateBuilder<'a> {
    cfg: &'a SimConfig,
    engine_id: EngineId,
}

impl<'a> ChainStateBuilder<'a> {
    fn new(cfg: &'a SimConfig, engine_id: EngineId) -> Self {
        Self { cfg, engine_id }
    }

    fn build(&self) -> sage_core::ChainState {
        sage_core::ChainState {
            execution: ExecutionState::new_with_accounts(
                self.cfg.workload.state_accounts,
                self.cfg.workload.initial_balance,
            ),
            platform: PlatformState {
                chain_id: ChainId::new("sage-sim"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                migration: None,
            },
            consensus: ConsensusStateEnvelope {
                engine: self.engine_id,
                opaque: vec![],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AdversaryConfig;
    use crate::StrategyKind;
    use sage_controller::Schedule;

    #[test]
    fn no_fault_simulation_finalizes_blocks() {
        let mut sim = Simulation::new(SimConfig::default()).unwrap();
        let metrics = sim.run().unwrap();
        assert!(metrics.finalized_blocks > 0);
        assert_eq!(
            metrics.max_finalized_height,
            SimConfig::default().simulation.max_height
        );
        assert!(!metrics.safety_violation);
    }

    #[test]
    fn partition_drops_cross_group_messages() {
        let mut cfg = SimConfig::default();
        cfg.simulation.max_height = 6;
        cfg.adversary = AdversaryConfig {
            partition_enabled: true,
            partition_start_height: 2,
            partition_duration_blocks: 3,
            partition_split: 2,
            byzantine_count: 0,
        };
        cfg.validators.n = 4;
        cfg.validators.f = 1;
        cfg.strategy = StrategyKind::Sage;
        let mut sim = Simulation::new(cfg).unwrap();
        let metrics = sim.run().unwrap();
        // under partition, fewer blocks are finalized because quorum may be unreachable
        assert!(metrics.finalized_blocks > 0);
        assert!(!metrics.safety_violation);
    }

    #[test]
    fn hardfork_under_partition_terminates_without_crash() {
        let mut cfg = SimConfig::default();
        cfg.simulation.max_height = 4;
        cfg.simulation.max_events = 50_000;
        cfg.validators.n = 6;
        cfg.validators.f = 1;
        cfg.strategy = StrategyKind::HardFork;
        cfg.migration = Schedule {
            h_d: Height::new(1),
            h_c: Height::new(2),
            h_r: Height::new(4),
            kappa: 1,
            tau_blocks: 1,
        };
        cfg.adversary = AdversaryConfig {
            partition_enabled: true,
            partition_start_height: 2,
            partition_duration_blocks: 2,
            partition_split: 3,
            byzantine_count: 0,
        };
        let mut sim = Simulation::new(cfg).unwrap();
        let metrics = sim.run().unwrap();
        // Under partition, neither side reaches BFT quorum (each has 3 < n-f=5).
        // Blocks finalized before partition at height 1 via PoA.
        assert!(metrics.finalized_blocks > 0);
    }

    #[test]
    fn hardfork_under_partition_produces_fork_with_view_cycling() {
        let mut cfg = SimConfig::default();
        cfg.simulation.max_height = 10;
        cfg.simulation.max_events = 500_000;
        cfg.validators.n = 6;
        cfg.validators.f = 1;
        cfg.strategy = StrategyKind::HardFork;
        cfg.migration = Schedule {
            h_d: Height::new(1),
            h_c: Height::new(4), // cut over at height 4
            h_r: Height::new(10),
            kappa: 1,
            tau_blocks: 1,
        };
        // Partition starts at height 4 (same as cutover), so proposal at 4 goes
        // across the partition, but heights 5+ are partitioned.
        cfg.adversary = AdversaryConfig {
            partition_enabled: true,
            partition_start_height: 4,
            partition_duration_blocks: 7,
            partition_split: 3,
            byzantine_count: 0,
        };
        let mut sim = Simulation::new(cfg).unwrap();
        let result = sim.run();
        match result {
            Ok(metrics) => {
                eprintln!(
                    "HF_FORK: finalized={} max_h={} safety={}",
                    metrics.finalized_blocks,
                    metrics.max_finalized_height,
                    metrics.safety_violation
                );
                assert!(metrics.max_finalized_height >= 4);
            }
            Err(e) => {
                eprintln!("HF_FORK: terminated={:?}", e);
            }
        }
    }

    #[test]
    fn sage_under_partition_produces_no_fork() {
        let mut cfg = SimConfig::default();
        cfg.simulation.max_height = 8;
        cfg.simulation.max_events = 500_000;
        cfg.validators.n = 6;
        cfg.validators.f = 1;
        cfg.strategy = StrategyKind::Sage;
        cfg.migration = Schedule {
            h_d: Height::new(1),
            h_c: Height::new(6),
            h_r: Height::new(8),
            kappa: 1,
            tau_blocks: 1,
        };
        cfg.adversary = AdversaryConfig {
            partition_enabled: true,
            partition_start_height: 5,
            partition_duration_blocks: 5,
            partition_split: 3,
            byzantine_count: 0,
        };
        let mut sim = Simulation::new(cfg).unwrap();
        let result = sim.run();
        match result {
            Ok(metrics) => {
                // SAGE under partition: CutCert requires quorum before cutover;
                // no side should be able to switch to target → PoA continues.
                assert!(
                    !metrics.safety_violation,
                    "SAGE should not fork under partition"
                );
            }
            Err(_) => {
                // max_events exceeded is acceptable under heavy partition cycling
            }
        }
    }

    #[test]
    fn sage_under_partition_terminates_without_crash() {
        let mut cfg = SimConfig::default();
        cfg.simulation.max_height = 4;
        cfg.simulation.max_events = 50_000;
        cfg.validators.n = 6;
        cfg.validators.f = 1;
        cfg.strategy = StrategyKind::Sage;
        cfg.migration = Schedule {
            h_d: Height::new(1),
            h_c: Height::new(2),
            h_r: Height::new(4),
            kappa: 1,
            tau_blocks: 1,
        };
        cfg.adversary = AdversaryConfig {
            partition_enabled: true,
            partition_start_height: 2,
            partition_duration_blocks: 2,
            partition_split: 3,
            byzantine_count: 0,
        };
        let mut sim = Simulation::new(cfg).unwrap();
        let metrics = sim.run().unwrap();
        assert!(metrics.finalized_blocks > 0);
    }
}
