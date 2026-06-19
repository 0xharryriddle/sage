use crate::Hash32;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashDomain {
    BlockHeaderV1,
    BlockV1,
    StateRootV1,
    VoteV1,
    CertificateV1,
    ManifestV1,
    ReadinessV1,
    AbortV1,
    TimeoutCertificateV1,
    Ed25519KeyV1,
}

impl HashDomain {
    fn tag(self) -> &'static [u8] {
        match self {
            Self::BlockHeaderV1 => b"block-header-v1",
            Self::BlockV1 => b"block-v1",
            Self::StateRootV1 => b"state-root-v1",
            Self::VoteV1 => b"vote-v1",
            Self::CertificateV1 => b"certificate-v1",
            Self::ManifestV1 => b"manifest-v1",
            Self::ReadinessV1 => b"readiness-v1",
            Self::AbortV1 => b"abort-v1",
            Self::TimeoutCertificateV1 => b"timeout-cert-v1",
            Self::Ed25519KeyV1 => b"ed25519-key-v1",
        }
    }
}

pub trait CanonicalEncode {
    fn encode_canonical(&self, out: &mut Vec<u8>);

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode_canonical(&mut out);
        out
    }
}

pub fn hash_domain(domain: HashDomain, bytes: &[u8]) -> Hash32 {
    let mut hasher = Sha256::new();
    hasher.update(b"SAGE");
    hasher.update([0]);
    hasher.update((domain.tag().len() as u64).to_be_bytes());
    hasher.update(domain.tag());
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    Hash32::new(hasher.finalize().into())
}

pub fn hash_canonical<T: CanonicalEncode>(domain: HashDomain, value: &T) -> Hash32 {
    hash_domain(domain, &value.canonical_bytes())
}

pub fn encode_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

pub fn encode_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}
pub fn encode_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains_separate_identical_payloads() {
        let payload = b"same";
        assert_ne!(
            hash_domain(HashDomain::BlockV1, payload),
            hash_domain(HashDomain::VoteV1, payload)
        );
    }

    #[test]
    fn hashing_is_deterministic() {
        let a = hash_domain(HashDomain::StateRootV1, b"abc");
        let b = hash_domain(HashDomain::StateRootV1, b"abc");
        assert_eq!(a, b);
    }
}
