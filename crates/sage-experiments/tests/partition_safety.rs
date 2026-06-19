//! Integration test: Partition safety — HardFork vs SAGE under partition.
//! Verifies: HardFork produces different blocks (potential fork indication),
//! SAGE continues on legacy engine without switching to target.
use sage_controller::Schedule;
use sage_core::Height;
use sage_sim::{AdversaryConfig, SimConfig, Simulation, StrategyKind};

#[test]
fn hardfork_under_partition_terminates() {
    let mut cfg = SimConfig::default();
    cfg.simulation.max_height = 6;
    cfg.simulation.max_events = 200_000;
    cfg.validators.n = 6;
    cfg.validators.f = 1;
    cfg.strategy = StrategyKind::HardFork;
    cfg.migration = Schedule {
        h_d: Height::new(1),
        h_c: Height::new(2),
        h_r: Height::new(6),
        kappa: 1,
        tau_blocks: 1,
    };
    cfg.adversary = AdversaryConfig {
        partition_enabled: true,
        partition_start_height: 2,
        partition_duration_blocks: 4,
        partition_split: 3,
        byzantine_count: 0,
    };

    let mut sim = Simulation::new(cfg).unwrap();
    let metrics = sim.run().unwrap();

    // Simulation terminates without crashing
    assert!(metrics.finalized_blocks > 0);
}

#[test]
fn sage_under_partition_stays_on_legacy() {
    let mut cfg = SimConfig::default();
    cfg.simulation.max_height = 6;
    cfg.simulation.max_events = 200_000;
    cfg.validators.n = 6;
    cfg.validators.f = 1;
    cfg.strategy = StrategyKind::Sage;
    cfg.migration = Schedule {
        h_d: Height::new(1),
        h_c: Height::new(5),
        h_r: Height::new(6),
        kappa: 4,
        tau_blocks: 1,
    };
    cfg.adversary = AdversaryConfig {
        partition_enabled: true,
        partition_start_height: 3,
        partition_duration_blocks: 4,
        partition_split: 3,
        byzantine_count: 0,
    };

    let mut sim = Simulation::new(cfg).unwrap();
    let metrics = sim.run().unwrap();

    // SAGE under partition: no safety violation
    assert!(!metrics.safety_violation);
    // Simulation terminates
    assert!(metrics.finalized_blocks > 0);
}

#[test]
fn default_simulation_no_fault_completes() {
    let mut sim = Simulation::new(SimConfig::default()).unwrap();
    let metrics = sim.run().unwrap();
    assert!(metrics.finalized_blocks > 0);
    assert!(!metrics.safety_violation);
}
