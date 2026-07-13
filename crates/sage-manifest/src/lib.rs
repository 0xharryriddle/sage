//! Migration manifest and certificate verification.

pub mod certificate;
pub mod error;
pub mod manifest;
pub mod signatures;
pub mod verify;

#[cfg(feature = "real-crypto")]
pub mod real_ed25519;

pub use certificate::{Certificate, CertificateKind, CertificatePayload, CutoverCertificate};
pub use error::{ManifestError, ManifestResult};
pub use manifest::{ManifestBuilder, MigrationManifest};
pub use signatures::{SignatureEnvelope, SignatureScheme, SimulatedSignatureScheme};
pub use verify::{ManifestVerifier, VerificationContext};
