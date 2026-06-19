use crate::{HotStuffMessage, PoaMessage};
use sage_core::{ChainId, ConfigId, Epoch, ValidatorId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusMessage {
    Poa(PoaMessage),
    HotStuff(HotStuffMessage),
    Sage(SageMessage),
}

/// SAGE migration-control messages, distinct from the consensus engines' own
/// traffic. The cutover attestation is how a validator signals "I have locally
/// validated the migration boundary and vote to cut over at `height` anchored to
/// `boundary_block`." Switching engines is gated on collecting a cutover QUORUM
/// (n-f) of distinct attestations for the same boundary — NOT on a single
/// validator's local readiness. This is the enforcement of SAGE's safety
/// invariant: two disjoint sets of size >= n-f require 2(n-f) <= n, i.e. n <= 2f,
/// contradicting BFT's n >= 2f+1, so at most one side of any partition can ever
/// form the cutover quorum. Hence at most one side switches engines and no
/// cross-engine fork can occur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SageMessage {
    CutoverAttestation {
        height: sage_core::Height,
        boundary_block: sage_core::BlockHash,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub from: ValidatorId,
    pub to: Option<ValidatorId>,
    pub chain_id: ChainId,
    pub epoch: Epoch,
    pub config_id: ConfigId,
    pub message: ConsensusMessage,
}
