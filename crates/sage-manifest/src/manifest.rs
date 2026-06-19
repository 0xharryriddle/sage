use crate::{
    Certificate, CertificateKind, ManifestError, ManifestResult, SignatureEnvelope, SignatureScheme,
};
use sage_core::crypto::{encode_bytes, encode_u64, hash_domain, HashDomain};
use sage_core::{
    BlockHash, ChainId, ConfigId, EngineId, Epoch, Hash32, Height, StateRoot, ValidatorId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationManifest {
    pub chain_id: ChainId,
    pub epoch: Epoch,
    pub config_id: ConfigId,
    pub cutover_height: Height,
    pub parent_hash: BlockHash,
    pub boundary_root: StateRoot,
    pub source_engine: EngineId,
    pub target_engine: EngineId,
    pub tool_hash: Hash32,
    pub cut_cert: Certificate,
    pub manifest_signature: SignatureEnvelope,
}

impl MigrationManifest {
    pub fn signing_payload(&self) -> Hash32 {
        let mut out = Vec::new();
        encode_bytes(&mut out, self.chain_id.as_str().as_bytes());
        encode_u64(&mut out, self.epoch.get());
        encode_u64(&mut out, self.config_id.get());
        encode_u64(&mut out, self.cutover_height.get());
        encode_bytes(&mut out, self.parent_hash.as_bytes());
        encode_bytes(&mut out, self.boundary_root.as_bytes());
        encode_u64(&mut out, self.source_engine.generation.get());
        encode_u64(&mut out, self.target_engine.generation.get());
        encode_bytes(&mut out, self.tool_hash.as_bytes());
        encode_bytes(&mut out, self.cut_cert.payload_hash().as_bytes());
        hash_domain(HashDomain::ManifestV1, &out)
    }
}

pub struct ManifestBuilder<S> {
    signature_scheme: S,
    signer: ValidatorId,
}

impl<S: SignatureScheme> ManifestBuilder<S> {
    pub fn new(signature_scheme: S, signer: ValidatorId) -> Self {
        Self {
            signature_scheme,
            signer,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &self,
        chain_id: ChainId,
        epoch: Epoch,
        config_id: ConfigId,
        cutover_height: Height,
        parent_hash: BlockHash,
        boundary_root: StateRoot,
        source_engine: EngineId,
        target_engine: EngineId,
        tool_hash: Hash32,
        cut_cert: Certificate,
    ) -> ManifestResult<MigrationManifest> {
        if cut_cert.payload.kind != CertificateKind::Cutover {
            return Err(ManifestError::InvalidManifest(
                "cut certificate must have Cutover kind".to_string(),
            ));
        }
        if cut_cert.payload.root != boundary_root {
            return Err(ManifestError::WrongBoundaryRoot {
                expected: boundary_root,
                actual: cut_cert.payload.root,
            });
        }
        let unsigned = MigrationManifest {
            chain_id,
            epoch,
            config_id,
            cutover_height,
            parent_hash,
            boundary_root,
            source_engine,
            target_engine,
            tool_hash,
            cut_cert,
            manifest_signature: SignatureEnvelope::Simulated {
                payload_hash: Hash32::ZERO,
            },
        };
        let manifest_signature = self
            .signature_scheme
            .sign(self.signer, unsigned.signing_payload())?;
        Ok(MigrationManifest {
            manifest_signature,
            ..unsigned
        })
    }
}
