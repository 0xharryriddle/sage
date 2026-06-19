//! Real cryptographic signature scheme using Ed25519.
//! Available behind the `real-crypto` feature flag.

use crate::{ManifestError, ManifestResult, SignatureEnvelope, SignatureScheme};
use sage_core::{Hash32, ValidatorId};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(feature = "real-crypto")]
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
#[cfg(feature = "real-crypto")]
use rand::rngs::OsRng;

/// Real Ed25519 signature scheme with per-validator keypairs.
#[cfg(feature = "real-crypto")]
#[derive(Default)]
pub struct RealEd25519Scheme {
    sign_keys: BTreeMap<ValidatorId, SigningKey>,
    verify_keys: BTreeMap<ValidatorId, VerifyingKey>,
}

#[cfg(feature = "real-crypto")]
impl RealEd25519Scheme {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, id: ValidatorId) {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        self.sign_keys.insert(id, signing_key);
        self.verify_keys.insert(id, verifying_key);
    }

    /// Register a validator with a DETERMINISTIC keypair derived from its id
    /// (and an optional domain seed). Every process that calls this with the
    /// same id derives the identical keypair, so a multi-process testbed needs
    /// no key-exchange / PKI: each validator can sign with its own key and
    /// verify peers' signatures purely from their ids. This is a deliberate
    /// testbed simplification (documented as a threat-to-validity); a real
    /// deployment would distribute public keys out of band.
    pub fn register_deterministic(&mut self, id: ValidatorId, seed: u64) {
        let mut material_in = Vec::with_capacity(12);
        material_in.extend_from_slice(&id.get().to_be_bytes());
        material_in.extend_from_slice(&seed.to_be_bytes());
        let material = sage_core::crypto::hash_domain(
            sage_core::crypto::HashDomain::Ed25519KeyV1,
            &material_in,
        );
        let signing_key = SigningKey::from_bytes(material.as_bytes());
        let verifying_key = signing_key.verifying_key();
        self.sign_keys.insert(id, signing_key);
        self.verify_keys.insert(id, verifying_key);
    }

    pub fn is_registered(&self, id: ValidatorId) -> bool {
        self.sign_keys.contains_key(&id)
    }
}

#[cfg(feature = "real-crypto")]
impl SignatureScheme for RealEd25519Scheme {
    fn sign(&self, signer: ValidatorId, payload: Hash32) -> ManifestResult<SignatureEnvelope> {
        let key = self
            .sign_keys
            .get(&signer)
            .ok_or(ManifestError::UnknownSigner { signer })?;
        let sig: Signature = key.sign(payload.as_bytes());
        Ok(SignatureEnvelope::Ed25519 {
            signer,
            payload_hash: payload,
            signature_bytes: sig.to_vec(),
        })
    }

    fn verify(
        &self,
        signers: &BTreeSet<ValidatorId>,
        payload: Hash32,
        signature: &SignatureEnvelope,
    ) -> ManifestResult<()> {
        match signature {
            SignatureEnvelope::Ed25519 {
                signer,
                signature_bytes,
                ..
            } if signers.contains(signer) => {
                let vk = self
                    .verify_keys
                    .get(signer)
                    .ok_or(ManifestError::UnknownSigner { signer: *signer })?;
                let sig = Signature::from_slice(signature_bytes)
                    .map_err(|_| ManifestError::PayloadMismatch)?;
                vk.verify(payload.as_bytes(), &sig)
                    .map_err(|_| ManifestError::PayloadMismatch)
            }
            SignatureEnvelope::Ed25519 { signer, .. } => {
                Err(ManifestError::UnknownSigner { signer: *signer })
            }
            _ => Err(ManifestError::PayloadMismatch),
        }
    }
}

#[cfg(all(test, feature = "real-crypto"))]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn real_ed25519_sign_and_verify() {
        let mut scheme = RealEd25519Scheme::new();
        scheme.register(ValidatorId::new(0));
        scheme.register(ValidatorId::new(1));

        let payload = Hash32::new([0x42; 32]);
        let sig = scheme.sign(ValidatorId::new(0), payload).unwrap();

        let signers: BTreeSet<ValidatorId> = [ValidatorId::new(0)].into_iter().collect();
        scheme.verify(&signers, payload, &sig).unwrap();
    }

    #[test]
    fn real_ed25519_wrong_payload_fails() {
        let mut scheme = RealEd25519Scheme::new();
        scheme.register(ValidatorId::new(0));

        let payload = Hash32::new([0x42; 32]);
        let sig = scheme.sign(ValidatorId::new(0), payload).unwrap();

        let wrong = Hash32::new([0xFF; 32]);
        let signers: BTreeSet<ValidatorId> = [ValidatorId::new(0)].into_iter().collect();
        assert!(scheme.verify(&signers, wrong, &sig).is_err());
    }

    #[test]
    fn real_ed25519_unknown_signer_fails() {
        let mut scheme = RealEd25519Scheme::new();
        scheme.register(ValidatorId::new(0));

        let payload = Hash32::new([0x42; 32]);
        assert!(scheme.sign(ValidatorId::new(99), payload).is_err());
    }
}
