use crate::{ManifestError, ManifestResult, SignatureEnvelope};
use sage_consensus::{QuorumThreshold, ValidatorSet};
use sage_core::crypto::{
    encode_bytes, encode_u32, encode_u64, hash_canonical, CanonicalEncode, HashDomain,
};
use sage_core::{
    BlockHash, ChainId, ConfigId, EngineId, Epoch, Hash32, Height, StateRoot, ValidatorId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CertificateKind {
    Finality,
    Readiness,
    Cutover,
    Abort,
    Checkpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificatePayload {
    pub chain_id: ChainId,
    pub epoch: Epoch,
    pub config_id: ConfigId,
    pub kind: CertificateKind,
    pub height: Height,
    pub root: StateRoot,
    pub block_hash: Option<BlockHash>,
    pub engine_id: EngineId,
}

impl CanonicalEncode for CertificatePayload {
    fn encode_canonical(&self, out: &mut Vec<u8>) {
        encode_bytes(out, self.chain_id.as_str().as_bytes());
        encode_u64(out, self.epoch.get());
        encode_u64(out, self.config_id.get());
        encode_u32(out, self.kind as u32);
        encode_u64(out, self.height.get());
        encode_bytes(out, self.root.as_bytes());
        match self.block_hash {
            Some(hash) => {
                out.push(1);
                encode_bytes(out, hash.as_bytes());
            }
            None => out.push(0),
        }
        encode_u32(out, self.engine_id.kind as u32);
        encode_u64(out, self.engine_id.generation.get());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Certificate {
    pub payload: CertificatePayload,
    pub signers: BTreeSet<ValidatorId>,
    pub signature: SignatureEnvelope,
}

impl Certificate {
    pub fn payload_hash(&self) -> Hash32 {
        hash_canonical(HashDomain::CertificateV1, &self.payload)
    }

    pub fn validate_signer_set(
        &self,
        validators: &ValidatorSet,
        required: QuorumThreshold,
    ) -> ManifestResult<()> {
        let actual = validators
            .power_of_signers(&self.signers)
            .map_err(|err| match err {
                sage_consensus::ConsensusError::UnknownValidator { validator } => {
                    ManifestError::UnknownSigner { signer: validator }
                }
                other => ManifestError::InvalidManifest(other.to_string()),
            })?;
        if actual < required.required_power {
            return Err(ManifestError::InsufficientQuorum {
                required: required.required_power,
                actual,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SignatureScheme;
    use crate::SimulatedSignatureScheme;
    use sage_consensus::QuorumPolicy;
    use sage_core::{EngineGeneration, EngineKind};

    #[test]
    fn rejects_insufficient_quorum() {
        let validators = ValidatorSet::equal_power(ConfigId::new(1), Epoch::new(1), 4);
        let payload = CertificatePayload {
            chain_id: ChainId::new("sage"),
            epoch: Epoch::new(1),
            config_id: ConfigId::new(1),
            kind: CertificateKind::Cutover,
            height: Height::new(9),
            root: Hash32::ZERO,
            block_hash: None,
            engine_id: EngineId::new(EngineKind::HotStuff, EngineGeneration::new(1)),
        };
        let hash = sage_core::crypto::hash_canonical(HashDomain::CertificateV1, &payload);
        let cert = Certificate {
            payload,
            signers: [ValidatorId::new(0)].into_iter().collect(),
            signature: SimulatedSignatureScheme
                .sign(ValidatorId::new(0), hash)
                .unwrap(),
        };
        let threshold = QuorumPolicy::MigrationCutover { f: 1 }
            .threshold(&validators)
            .unwrap();
        assert!(matches!(
            cert.validate_signer_set(&validators, threshold),
            Err(ManifestError::InsufficientQuorum { .. })
        ));
    }
}
