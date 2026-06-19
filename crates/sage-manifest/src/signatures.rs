use crate::{ManifestError, ManifestResult};
use sage_core::Hash32;
use sage_core::ValidatorId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureEnvelope {
    Simulated {
        payload_hash: Hash32,
    },
    #[cfg(feature = "real-crypto")]
    Ed25519 {
        signer: ValidatorId,
        payload_hash: Hash32,
        signature_bytes: Vec<u8>,
    },
}

pub trait SignatureScheme {
    fn sign(&self, signer: ValidatorId, payload: Hash32) -> ManifestResult<SignatureEnvelope>;
    fn verify(
        &self,
        signers: &BTreeSet<ValidatorId>,
        payload: Hash32,
        signature: &SignatureEnvelope,
    ) -> ManifestResult<()>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SimulatedSignatureScheme;

impl SignatureScheme for SimulatedSignatureScheme {
    fn sign(&self, _signer: ValidatorId, payload: Hash32) -> ManifestResult<SignatureEnvelope> {
        Ok(SignatureEnvelope::Simulated {
            payload_hash: payload,
        })
    }

    fn verify(
        &self,
        _signers: &BTreeSet<ValidatorId>,
        payload: Hash32,
        signature: &SignatureEnvelope,
    ) -> ManifestResult<()> {
        match signature {
            SignatureEnvelope::Simulated { payload_hash } if *payload_hash == payload => Ok(()),
            SignatureEnvelope::Simulated { .. } => Err(ManifestError::PayloadMismatch),
            #[cfg(feature = "real-crypto")]
            _ => Err(ManifestError::PayloadMismatch),
        }
    }
}
