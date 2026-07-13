//! Cutover-certificate TIMING benchmark (system-upgrade P1.2).
//!
//! Complements `run_cert_sizes` (which measures the wire SIZE of an `n-f`
//! cutover certificate) by measuring the real CPU COST of producing and
//! verifying that certificate as a function of `n`, using actual Ed25519
//! signatures (the `real-crypto` scheme). This turns the paper's "the one-shot
//! cutover is cheap even though the certificate is O(n)" claim into a measured
//! table rather than a hand-wave toward BLS/FROST.
//!
//! What is measured (under `--features real-crypto`):
//! - `sign_total_us`: wall time to produce all `n-f` Ed25519 signatures over the
//!   shared cutover tuple (the cost the collector's peers pay, summed).
//! - `verify_all_p50_us` / `verify_all_p95_us`: wall time to verify all `n-f`
//!   signatures, reported as p50/p95 over many iterations (the cost a validator
//!   pays once at the boundary to accept the CutCert).
//! - `cert_bytes`: the serialized multi-signature size (matches run_cert_sizes).
//!
//! What is MODELED (clearly labeled, not measured):
//! - `gather_model_us`: a single post-GST round-trip at the swept `n`, i.e. the
//!   analytic one-round gather cost. This is NOT a live-network measurement; it
//!   is the `Delta_cert` term of Proposition (boundary latency) instantiated at
//!   a nominal one-way delay, provided only so the table has an end-to-end
//!   order-of-magnitude. Real gather latency appears in the multi-process and
//!   real-host tiers, not here.
//!
//! Honesty notes:
//! - The default build (without `real-crypto`) cannot measure real signatures,
//!   so the timing columns are 0 and `measurement_source=analytic_model_only`.
//!   Run with `--features real-crypto` for the measured columns.
//!
//! Build/run:
//!   cargo run -p sage-experiments --features real-crypto --bin run_cert_timing

use clap::Parser;
use sage_experiments::common::write_csv;
use sage_experiments::error::ExperimentResult;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "results/raw")]
    out_dir: PathBuf,
    #[arg(long, value_delimiter = ',', default_value = "31,64,100,200")]
    validators: Vec<u32>,
    /// Iterations for the verify-all p50/p95 distribution.
    #[arg(long, default_value_t = 1000)]
    iters: u32,
    /// Nominal one-way delay (ms) used only for the MODELED gather round-trip.
    #[arg(long, default_value_t = 25)]
    gather_one_way_ms: u64,
}

#[derive(Debug, Serialize)]
struct CertTimingRow {
    experiment: String,
    n: u32,
    f: u64,
    signers: u64,
    /// MEASURED total wall time to produce all n-f Ed25519 signatures (us).
    sign_total_us: u64,
    /// MEASURED p50 wall time to verify all n-f signatures (us).
    verify_all_p50_us: u64,
    /// MEASURED p95 wall time to verify all n-f signatures (us).
    verify_all_p95_us: u64,
    /// MEASURED serialized multi-signature size (bytes).
    cert_bytes: usize,
    /// MODELED one-round gather cost = 2 * one_way_delay (us). Not measured.
    gather_model_us: u64,
    measurement_source: String,
}

#[cfg(not(feature = "real-crypto"))]
const ED25519_SIG_BYTES: usize = 64;
const SIGNER_ID_BYTES: usize = 8;

#[cfg(feature = "real-crypto")]
fn measure(signers: u64, iters: u32) -> (u64, u64, u64, usize) {
    use sage_core::{Hash32, ValidatorId};
    use sage_manifest::real_ed25519::RealEd25519Scheme;
    use sage_manifest::{SignatureEnvelope, SignatureScheme};
    use std::collections::BTreeSet;
    use std::time::Instant;

    let mut scheme = RealEd25519Scheme::new();
    let payload = Hash32::new([0x5a; 32]);
    for i in 0..signers {
        scheme.register_deterministic(ValidatorId::new(i as u32), 0);
    }

    // --- sign_total: produce all n-f signatures once, timed. ---
    let mut envelopes = Vec::with_capacity(signers as usize);
    let mut cert_bytes = 0usize;
    let t_sign = Instant::now();
    for i in 0..signers {
        let id = ValidatorId::new(i as u32);
        let env = scheme
            .sign(id, payload)
            .expect("deterministic signer registered");
        envelopes.push((id, env));
    }
    let sign_total_us = t_sign.elapsed().as_micros() as u64;
    for (_, env) in &envelopes {
        if let SignatureEnvelope::Ed25519 {
            signature_bytes, ..
        } = env
        {
            cert_bytes += signature_bytes.len() + SIGNER_ID_BYTES;
        }
    }

    // --- verify_all distribution over `iters` iterations. ---
    let mut samples: Vec<u64> = Vec::with_capacity(iters as usize);
    for _ in 0..iters {
        let t = Instant::now();
        for (id, env) in &envelopes {
            let signers_set: BTreeSet<ValidatorId> = [*id].into_iter().collect();
            scheme
                .verify(&signers_set, payload, env)
                .expect("valid signature verifies");
        }
        samples.push(t.elapsed().as_micros() as u64);
    }
    samples.sort_unstable();
    let p = |q: f64| -> u64 {
        if samples.is_empty() {
            return 0;
        }
        let idx = ((samples.len() as f64 - 1.0) * q).round() as usize;
        samples[idx]
    };
    (sign_total_us, p(0.50), p(0.95), cert_bytes)
}

#[cfg(not(feature = "real-crypto"))]
fn measure(signers: u64, _iters: u32) -> (u64, u64, u64, usize) {
    // Analytic size only; no real signatures without the feature.
    (
        0,
        0,
        0,
        (ED25519_SIG_BYTES + SIGNER_ID_BYTES) * signers as usize,
    )
}

fn main() -> ExperimentResult<()> {
    let cli = Cli::parse();
    std::fs::create_dir_all(&cli.out_dir)?;

    let source = if cfg!(feature = "real-crypto") {
        "ed25519_real_signatures"
    } else {
        "analytic_model_only"
    };
    let gather_model_us = cli.gather_one_way_ms * 2 * 1000;

    let mut rows = Vec::new();
    for &n in &cli.validators {
        let f = ((n.saturating_sub(1)) / 3) as u64;
        if f == 0 || (n as u64) <= 3 * f {
            eprintln!("skipping invalid BFT size n={n}, f={f}");
            continue;
        }
        let signers = n as u64 - f;
        let (sign_total_us, p50, p95, cert_bytes) = measure(signers, cli.iters);
        rows.push(CertTimingRow {
            experiment: "cert_timing".into(),
            n,
            f,
            signers,
            sign_total_us,
            verify_all_p50_us: p50,
            verify_all_p95_us: p95,
            cert_bytes,
            gather_model_us,
            measurement_source: source.into(),
        });
    }

    let csv_path = cli.out_dir.join("cert_timing.csv");
    write_csv(&csv_path, &rows)?;
    println!(
        "Wrote {} rows to {} (source: {}, iters: {})",
        rows.len(),
        csv_path.display(),
        source,
        cli.iters
    );
    Ok(())
}
