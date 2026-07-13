//! Cutover-certificate size measurement (P2-G).
//!
//! Measures the REAL wire cost of an `n-f` cutover certificate as a function of
//! `n`, using actual Ed25519 signatures (the `real-crypto` scheme), and contrasts
//! it with the constant-size BLS/threshold aggregate a production deployment would
//! prefer. This turns the paper's O(n) vs O(1) certificate claim into a measured
//! table rather than an assertion.
//!
//! Honesty notes:
//! - The Ed25519 column is MEASURED: we sign a real payload with `n-f` distinct
//!   deterministic keypairs and serialize each 64-byte signature plus its signer id.
//! - The BLS column is COMPUTED, not measured: a BLS12-381 aggregate signature is
//!   48 bytes and a participation bitmap is ceil(n/8) bytes. We do not pull in a
//!   pairing library here; we report the analytic constant-plus-bitmap size and
//!   label it as the target, not an implemented artifact.
//!
//! Build/run (real signatures require the feature flag):
//!   cargo run -p sage-experiments --features real-crypto --bin run_cert_sizes
//!
//! Without the feature, the binary still reports the analytic byte model so the
//! default build is not broken, but the MEASURED column is only populated under
//! `--features real-crypto`.
use clap::Parser;
use sage_experiments::common::write_csv;
use sage_experiments::error::ExperimentResult;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "results/raw")]
    out_dir: PathBuf,
    #[arg(
        long,
        value_delimiter = ',',
        default_value = "4,7,10,16,25,40,64,100,150"
    )]
    validators: Vec<u32>,
}

#[derive(Debug, Serialize)]
struct CertSizeRow {
    experiment: String,
    n: u32,
    f: u64,
    signers: u64,
    /// MEASURED serialized bytes of the n-f Ed25519 multi-signature payload
    /// (signature bytes + signer ids), or 0 if built without `real-crypto`.
    ed25519_measured_bytes: usize,
    /// ANALYTIC Ed25519 model: (64 sig + 8 signer-id) * (n-f) bytes.
    ed25519_model_bytes: usize,
    /// COMPUTED BLS12-381 aggregate target: 48-byte aggregate + ceil(n/8) bitmap.
    bls_aggregate_bytes: usize,
    /// Ratio of measured (or modeled) Ed25519 size to the BLS aggregate target.
    ed25519_over_bls: f64,
    measurement_source: String,
}

const ED25519_SIG_BYTES: usize = 64;
const SIGNER_ID_BYTES: usize = 8;
const BLS_AGG_BYTES: usize = 48;

fn bls_aggregate_bytes(n: u32) -> usize {
    BLS_AGG_BYTES + (n as usize).div_ceil(8)
}

fn ed25519_model_bytes(signers: u64) -> usize {
    (ED25519_SIG_BYTES + SIGNER_ID_BYTES) * signers as usize
}

#[cfg(feature = "real-crypto")]
fn ed25519_measured_bytes(signers: u64) -> usize {
    use sage_core::{Hash32, ValidatorId};
    use sage_manifest::real_ed25519::RealEd25519Scheme;
    use sage_manifest::{SignatureEnvelope, SignatureScheme};

    let mut scheme = RealEd25519Scheme::new();
    let payload = Hash32::new([0x5a; 32]);
    let mut total = 0usize;
    for i in 0..signers {
        let id = ValidatorId::new(i as u32);
        scheme.register_deterministic(id, 0);
        let env = scheme
            .sign(id, payload)
            .expect("deterministic signer is registered");
        // Serialize exactly what would travel on the wire for this signer's share.
        match env {
            SignatureEnvelope::Ed25519 {
                signature_bytes, ..
            } => {
                total += signature_bytes.len() + SIGNER_ID_BYTES;
            }
            _ => unreachable!("real-crypto scheme yields Ed25519 envelopes"),
        }
    }
    total
}

#[cfg(not(feature = "real-crypto"))]
fn ed25519_measured_bytes(_signers: u64) -> usize {
    0
}

fn main() -> ExperimentResult<()> {
    let cli = Cli::parse();
    std::fs::create_dir_all(&cli.out_dir)?;

    let source = if cfg!(feature = "real-crypto") {
        "ed25519_real_signatures"
    } else {
        "analytic_model_only"
    };

    let mut rows = Vec::new();
    for &n in &cli.validators {
        let f = ((n.saturating_sub(1)) / 3) as u64;
        if f == 0 || (n as u64) <= 3 * f {
            eprintln!("skipping invalid BFT size n={n}, f={f}");
            continue;
        }
        let signers = n as u64 - f; // n-f cutover quorum
        let measured = ed25519_measured_bytes(signers);
        let model = ed25519_model_bytes(signers);
        let bls = bls_aggregate_bytes(n);
        // Prefer the measured value when available; else fall back to the model
        // for the ratio so the column is never misleadingly zero.
        let numerator = if measured > 0 { measured } else { model } as f64;
        rows.push(CertSizeRow {
            experiment: "cert_sizes".into(),
            n,
            f,
            signers,
            ed25519_measured_bytes: measured,
            ed25519_model_bytes: model,
            bls_aggregate_bytes: bls,
            ed25519_over_bls: numerator / bls as f64,
            measurement_source: source.into(),
        });
    }

    let csv_path = cli.out_dir.join("cert_sizes.csv");
    write_csv(&csv_path, &rows)?;
    println!(
        "Wrote {} rows to {} (source: {})",
        rows.len(),
        csv_path.display(),
        source
    );
    Ok(())
}
