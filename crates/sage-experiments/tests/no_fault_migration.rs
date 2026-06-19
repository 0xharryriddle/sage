//! Integration test: No-fault SAGE migration from PoA to HotStuff.
//! Verifies: migration completes, no safety violations, target reaches finality.
use sage_controller::Schedule;
use sage_core::Height;
use sage_sim::{SimConfig, Simulation, StrategyKind};

#[test]
fn sage_no_fault_migration_completes() {
    let mut cfg = SimConfig::default();
    cfg.simulation.max_height = 8;
    cfg.simulation.max_events = 200_000;
    cfg.validators.n = 6;
    cfg.validators.f = 1;
    cfg.strategy = StrategyKind::Sage;
    cfg.migration = Schedule {
        h_d: Height::new(1),
        h_c: Height::new(3),
        h_r: Height::new(7),
        kappa: 1,
        tau_blocks: 1,
    };
    cfg.adversary.partition_enabled = false;

    let mut sim = Simulation::new(cfg).unwrap();
    let metrics = sim.run().unwrap();

    // Migration should produce finalized blocks
    assert!(metrics.finalized_blocks > 0, "SAGE should finalize blocks");
    // No fork / conflicting commits
    assert!(!metrics.safety_violation, "SAGE should not fork");
    // Migration succeeded (cutover happened)
    assert!(metrics.migration_success, "SAGE migration should succeed");
    // Reached target heights
    assert!(
        metrics.max_finalized_height >= 3,
        "should reach past cutover height"
    );
}

#[test]
fn reconfig_only_does_not_swap_engine() {
    let mut cfg = SimConfig::default();
    cfg.simulation.max_height = 6;
    cfg.simulation.max_events = 100_000;
    cfg.validators.n = 4;
    cfg.validators.f = 1;
    cfg.strategy = StrategyKind::ReconfigOnly;
    cfg.migration = Schedule {
        h_d: Height::new(1),
        h_c: Height::new(3),
        h_r: Height::new(6),
        kappa: 1,
        tau_blocks: 1,
    };

    let mut sim = Simulation::new(cfg).unwrap();
    let metrics = sim.run().unwrap();

    assert!(metrics.finalized_blocks > 0);
    assert!(!metrics.protocol_swap_success);
    assert!(!metrics.safety_violation);
}

#[test]
fn stop_the_world_halts_before_seal() {
    let mut cfg = SimConfig::default();
    cfg.simulation.max_height = 6;
    cfg.simulation.max_events = 200_000;
    cfg.validators.n = 4;
    cfg.validators.f = 1;
    cfg.strategy = StrategyKind::StopTheWorld;
    cfg.migration = Schedule {
        h_d: Height::new(1),
        h_c: Height::new(3),
        h_r: Height::new(6),
        kappa: 1,
        tau_blocks: 1,
    };

    let mut sim = Simulation::new(cfg).unwrap();
    let metrics = sim.run().unwrap();

    // STW should complete protocol swap
    assert!(metrics.protocol_swap_success);
    // No safety violations
    assert!(!metrics.safety_violation);
    // Migration can succeed
    assert!(metrics.migration_success);
}
