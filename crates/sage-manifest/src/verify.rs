use crate::{CertificateKind, ManifestError, ManifestResult, MigrationManifest, SignatureScheme};
use sage_consensus::{QuorumThreshold, ValidatorSet};
use sage_core::{BlockHash, ChainId, ConfigId, EngineId, Epoch, StateRoot};
use std::collections::BTreeSet;

pub struct VerificationContext<'a> {
    pub local_chain_id: &'a ChainId,
    pub local_epoch: Epoch,
    pub local_config_id: ConfigId,
    pub expected_source_engine: EngineId,
    pub expected_target_engine: EngineId,
    pub local_boundary_root: StateRoot,
    pub local_boundary_block_hash: BlockHash,
    pub local_parent_hash: BlockHash,
    pub validators: &'a ValidatorSet,
    pub required_cutover_quorum: QuorumThreshold,
    pub legacy_finalized_header_hashes: &'a BTreeSet<BlockHash>,
}

pub struct ManifestVerifier<S> {
    pub sigs: S,
}

impl<S: SignatureScheme> ManifestVerifier<S> {
    pub fn verify(
        &self,
        manifest: &MigrationManifest,
        ctx: &VerificationContext<'_>,
    ) -> ManifestResult<()> {
        if &manifest.chain_id != ctx.local_chain_id {
            return Err(ManifestError::WrongChainId {
                expected: ctx.local_chain_id.clone(),
                actual: manifest.chain_id.clone(),
            });
        }
        if manifest.epoch != ctx.local_epoch {
            return Err(ManifestError::WrongEpoch {
                expected: ctx.local_epoch,
                actual: manifest.epoch,
            });
        }
        if manifest.config_id != ctx.local_config_id {
            return Err(ManifestError::WrongConfigId {
                expected: ctx.local_config_id,
                actual: manifest.config_id,
            });
        }
        if manifest.source_engine != ctx.expected_source_engine {
            return Err(ManifestError::WrongEngine {
                expected: ctx.expected_source_engine,
                actual: manifest.source_engine,
            });
        }
        if manifest.target_engine != ctx.expected_target_engine {
            return Err(ManifestError::WrongEngine {
                expected: ctx.expected_target_engine,
                actual: manifest.target_engine,
            });
        }
        if manifest.boundary_root != ctx.local_boundary_root {
            return Err(ManifestError::WrongBoundaryRoot {
                expected: ctx.local_boundary_root,
                actual: manifest.boundary_root,
            });
        }
        if manifest.parent_hash != ctx.local_parent_hash
            || !ctx
                .legacy_finalized_header_hashes
                .contains(&manifest.parent_hash)
        {
            return Err(ManifestError::ManifestNotLegacyFinalized);
        }
        let cert = &manifest.cut_cert.payload;
        if cert.kind != CertificateKind::Cutover {
            return Err(ManifestError::InvalidManifest(
                "cut certificate must have Cutover kind".into(),
            ));
        }
        if cert.chain_id != manifest.chain_id
            || cert.epoch != manifest.epoch
            || cert.config_id != manifest.config_id
            || cert.height != manifest.cutover_height
            || cert.root != manifest.boundary_root
            || cert.engine_id != manifest.target_engine
            || cert.block_hash != Some(ctx.local_boundary_block_hash)
        {
            return Err(ManifestError::InvalidManifest(
                "cut certificate payload does not match manifest boundary".into(),
            ));
        }
        // The current envelope carries one individual signature, not an
        // aggregate. Never grant quorum power to declared IDs that did not
        // produce the enclosed Ed25519 signature.
        #[cfg(feature = "real-crypto")]
        if let crate::SignatureEnvelope::Ed25519 { signer, .. } = &manifest.cut_cert.signature {
            let authenticated = BTreeSet::from([*signer]);
            if manifest.cut_cert.signers != authenticated {
                return Err(ManifestError::InvalidManifest(
                    "cut certificate signer set is not covered by its signature".into(),
                ));
            }
        }
        manifest
            .cut_cert
            .validate_signer_set(ctx.validators, ctx.required_cutover_quorum)?;
        self.sigs.verify(
            &manifest.cut_cert.signers,
            manifest.cut_cert.payload_hash(),
            &manifest.cut_cert.signature,
        )?;
        self.sigs.verify(
            &manifest.cut_cert.signers,
            manifest.signing_payload(),
            &manifest.manifest_signature,
        )?;
        Ok(())
    }
}
