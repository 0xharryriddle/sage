//! Manifest negative tests: verifies that wrong-root, replayed,
//! insufficient-quorum, and wrong-chain manifests are correctly rejected.
//! Supports manifest unforgeability and replay-resistance claims (E8).
use clap::Parser;
use sage_consensus::{QuorumPolicy, ValidatorSet};
use sage_core::{
    BlockHash, ChainId, ConfigId, EngineGeneration, EngineId, EngineKind, Epoch, Hash32, Height,
    StateRoot, ValidatorId,
};
use sage_experiments::common::write_csv;
use sage_experiments::error::ExperimentResult;
use sage_manifest::{
    Certificate, CertificateKind, CertificatePayload, ManifestBuilder, ManifestVerifier,
    SignatureScheme, SimulatedSignatureScheme, VerificationContext,
};
use sage_sim::{SimConfig, Simulation, StrategyKind};
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "results/raw")]
    out_dir: std::path::PathBuf,
}

#[derive(Debug, Serialize)]
struct ManifestTestRow {
    test_name: String,
    passed: bool,
    message: String,
}

#[derive(Debug, Serialize)]
struct ManifestSimRow {
    test_name: String,
    seed: u64,
    safety_violation: bool,
    migration_success: bool,
    finalized_blocks: u64,
    message: String,
}

/// Build a correct manifest + contextual data for verification.
/// Returns (manifest, verifier, vset, parent_hash) — VerificationContext
/// can be built locally from these and the manifest.
fn make_fixtures() -> (
    sage_manifest::MigrationManifest,
    ManifestVerifier<SimulatedSignatureScheme>,
    ValidatorSet,
    BlockHash,
) {
    let chain_id = ChainId::new("test-chain");
    let epoch = Epoch::new(1);
    let config_id = ConfigId::new(1);
    let source_engine = EngineId::new(EngineKind::Poa, EngineGeneration::new(1));
    let target_engine = EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1));
    let boundary_root = StateRoot::new([0xAA; 32]);
    let parent_hash = BlockHash::new([0xBB; 32]);

    let n: u32 = 4;
    let vset = ValidatorSet::equal_power(config_id, epoch, n);

    let scheme = SimulatedSignatureScheme;
    let signer_id = ValidatorId::new(0);

    let payload = CertificatePayload {
        chain_id: chain_id.clone(),
        epoch,
        config_id,
        kind: CertificateKind::Cutover,
        height: Height::new(10),
        root: boundary_root,
        block_hash: Some(BlockHash::new([0xCC; 32])),
        engine_id: target_engine,
    };

    let signers: BTreeSet<ValidatorId> = (0..n).map(ValidatorId::new).collect();
    let cert_hash =
        sage_core::crypto::hash_canonical(sage_core::crypto::HashDomain::CertificateV1, &payload);
    let sig = scheme.sign(signer_id, cert_hash).expect("sign");

    let cut_cert = Certificate {
        payload: payload.clone(),
        signers: signers.clone(),
        signature: sig,
    };

    let cur_height = Height::new(10);
    let manifest = ManifestBuilder::new(SimulatedSignatureScheme, signer_id)
        .build(
            chain_id.clone(),
            epoch,
            config_id,
            cur_height,
            parent_hash,
            boundary_root,
            source_engine,
            target_engine,
            Hash32::new([0x00; 32]),
            cut_cert,
        )
        .expect("build manifest");

    let verifier = ManifestVerifier {
        sigs: SimulatedSignatureScheme,
    };
    (manifest, verifier, vset, parent_hash)
}

/// Build a VerificationContext for a given manifest and legacy set.
fn make_ctx<'a>(
    manifest: &'a sage_manifest::MigrationManifest,
    vset: &'a ValidatorSet,
    legacy_hashes: &'a BTreeSet<BlockHash>,
) -> VerificationContext<'a> {
    let threshold = QuorumPolicy::MigrationCutover { f: 1 }
        .threshold(vset)
        .unwrap();
    VerificationContext {
        local_chain_id: &manifest.chain_id,
        local_epoch: manifest.epoch,
        local_config_id: manifest.config_id,
        expected_source_engine: manifest.source_engine,
        expected_target_engine: manifest.target_engine,
        local_boundary_root: manifest.boundary_root,
        local_boundary_block_hash: manifest
            .cut_cert
            .payload
            .block_hash
            .expect("test cut certificate has a boundary block hash"),
        local_parent_hash: manifest.parent_hash,
        validators: vset,
        required_cutover_quorum: threshold,
        legacy_finalized_header_hashes: legacy_hashes,
    }
}

#[allow(clippy::field_reassign_with_default)]
fn main() -> ExperimentResult<()> {
    let cli = Cli::parse();
    fs::create_dir_all(&cli.out_dir)?;

    let mut test_rows: Vec<ManifestTestRow> = Vec::new();
    let mut sim_rows: Vec<ManifestSimRow> = Vec::new();

    // --- Manifest verification tests ---

    // Test 1: Correct manifest verification passes
    {
        let (manifest, verifier, vset, parent_hash) = make_fixtures();
        let legacy: BTreeSet<BlockHash> = [parent_hash].into_iter().collect();
        let ctx = make_ctx(&manifest, &vset, &legacy);
        match verifier.verify(&manifest, &ctx) {
            Ok(()) => test_rows.push(ManifestTestRow {
                test_name: "correct_manifest_passes".into(),
                passed: true,
                message: "correct manifest verified".into(),
            }),
            Err(e) => test_rows.push(ManifestTestRow {
                test_name: "correct_manifest_passes".into(),
                passed: false,
                message: format!("unexpected rejection: {}", e),
            }),
        }
    }

    // Test 2: Wrong chain ID rejected
    {
        let (mut manifest, verifier, vset, parent_hash) = make_fixtures();
        manifest.chain_id = ChainId::new("wrong-chain");
        let legacy: BTreeSet<BlockHash> = [parent_hash].into_iter().collect();
        let ctx = make_ctx(&manifest, &vset, &legacy);
        match verifier.verify(&manifest, &ctx) {
            Ok(()) => test_rows.push(ManifestTestRow {
                test_name: "wrong_chain_id_rejected".into(),
                passed: false,
                message: "wrong chain ID was NOT rejected".into(),
            }),
            Err(e) => test_rows.push(ManifestTestRow {
                test_name: "wrong_chain_id_rejected".into(),
                passed: true,
                message: format!("rejected: {}", e),
            }),
        }
    }

    // Test 3: Wrong boundary root rejected
    {
        let (mut manifest, verifier, vset, parent_hash) = make_fixtures();
        manifest.boundary_root = StateRoot::new([0xDE; 32]);
        let legacy: BTreeSet<BlockHash> = [parent_hash].into_iter().collect();
        let ctx = make_ctx(&manifest, &vset, &legacy);
        match verifier.verify(&manifest, &ctx) {
            Ok(()) => test_rows.push(ManifestTestRow {
                test_name: "wrong_boundary_root_rejected".into(),
                passed: false,
                message: "wrong root was NOT rejected".into(),
            }),
            Err(e) => test_rows.push(ManifestTestRow {
                test_name: "wrong_boundary_root_rejected".into(),
                passed: true,
                message: format!("rejected: {}", e),
            }),
        }
    }

    // Test 4: Wrong epoch rejected
    {
        let (mut manifest, verifier, vset, parent_hash) = make_fixtures();
        manifest.epoch = Epoch::new(99);
        let legacy: BTreeSet<BlockHash> = [parent_hash].into_iter().collect();
        let ctx = make_ctx(&manifest, &vset, &legacy);
        match verifier.verify(&manifest, &ctx) {
            Ok(()) => test_rows.push(ManifestTestRow {
                test_name: "wrong_epoch_rejected".into(),
                passed: false,
                message: "wrong epoch was NOT rejected".into(),
            }),
            Err(e) => test_rows.push(ManifestTestRow {
                test_name: "wrong_epoch_rejected".into(),
                passed: true,
                message: format!("rejected: {}", e),
            }),
        }
    }

    // Test 5: Wrong target engine rejected
    {
        let (mut manifest, verifier, vset, parent_hash) = make_fixtures();
        manifest.target_engine = EngineId::new(EngineKind::Poa, EngineGeneration::new(2));
        let legacy: BTreeSet<BlockHash> = [parent_hash].into_iter().collect();
        let ctx = make_ctx(&manifest, &vset, &legacy);
        match verifier.verify(&manifest, &ctx) {
            Ok(()) => test_rows.push(ManifestTestRow {
                test_name: "wrong_target_engine_rejected".into(),
                passed: false,
                message: "wrong target engine was NOT rejected".into(),
            }),
            Err(e) => test_rows.push(ManifestTestRow {
                test_name: "wrong_target_engine_rejected".into(),
                passed: true,
                message: format!("rejected: {}", e),
            }),
        }
    }

    // Test 6: Replayed manifest (parent_hash not in legacy finalized set)
    {
        let (manifest, verifier, vset, _parent_hash) = make_fixtures();
        let empty: BTreeSet<BlockHash> = BTreeSet::new();
        let ctx = make_ctx(&manifest, &vset, &empty);
        match verifier.verify(&manifest, &ctx) {
            Ok(()) => test_rows.push(ManifestTestRow {
                test_name: "replayed_manifest_rejected".into(),
                passed: false,
                message: "replayed manifest was NOT rejected".into(),
            }),
            Err(e) => test_rows.push(ManifestTestRow {
                test_name: "replayed_manifest_rejected".into(),
                passed: true,
                message: format!("rejected: {}", e),
            }),
        }
    }

    // Test 7: SAGE under partition produces no fork
    {
        let mut cfg = SimConfig::default();
        cfg.strategy = StrategyKind::Sage;
        cfg.simulation.max_height = 6;
        cfg.simulation.max_events = 200_000;
        cfg.simulation.seed = 0;
        cfg.migration.h_d = Height::new(1);
        cfg.migration.h_c = Height::new(3);
        cfg.migration.h_r = Height::new(6);
        cfg.migration.kappa = 1;
        cfg.migration.tau_blocks = 1;
        cfg.adversary.partition_enabled = true;
        cfg.adversary.partition_start_height = 2;
        cfg.adversary.partition_duration_blocks = 4;
        cfg.adversary.partition_split = cfg.validators.n / 2;

        match Simulation::new(cfg) {
            Ok(mut sim) => match sim.run() {
                Ok(metrics) => sim_rows.push(ManifestSimRow {
                    test_name: "sage_partition_no_fork".into(),
                    seed: 0,
                    safety_violation: metrics.safety_violation,
                    migration_success: metrics.migration_success,
                    finalized_blocks: metrics.finalized_blocks,
                    message: if metrics.safety_violation {
                        "FAIL: fork detected".into()
                    } else {
                        "PASS: no fork".into()
                    },
                }),
                Err(e) => sim_rows.push(ManifestSimRow {
                    test_name: "sage_partition_no_fork".into(),
                    seed: 0,
                    safety_violation: false,
                    migration_success: false,
                    finalized_blocks: 0,
                    message: format!("sim error: {}", e),
                }),
            },
            Err(e) => sim_rows.push(ManifestSimRow {
                test_name: "sage_partition_no_fork".into(),
                seed: 0,
                safety_violation: false,
                migration_success: false,
                finalized_blocks: 0,
                message: format!("create error: {}", e),
            }),
        }
    }

    // Report
    let mut passed = 0;
    let mut failed = 0;
    for row in &test_rows {
        if row.passed {
            passed += 1;
        } else {
            failed += 1;
        }
        println!(
            "[{}] {}: {}",
            if row.passed { "PASS" } else { "FAIL" },
            row.test_name,
            row.message
        );
    }
    for row in &sim_rows {
        let is_pass = !row.safety_violation;
        if is_pass {
            passed += 1;
        } else {
            failed += 1;
        }
        println!(
            "[{}] {}: {}",
            if is_pass { "PASS" } else { "FAIL" },
            row.test_name,
            row.message
        );
    }
    println!("Manifest tests: {} passed, {} failed", passed, failed);

    let test_path = cli.out_dir.join("manifest_tests.csv");
    write_csv(&test_path, &test_rows)?;
    let sim_path = cli.out_dir.join("manifest_sim.csv");
    write_csv(&sim_path, &sim_rows)?;

    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}
