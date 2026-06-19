//! Network transport abstraction.
//! Separates message-passing from protocol logic so the same consensus
//! code can run against an in-memory channel, TCP, QUIC, or simulator.

use crate::error::NetworkResult;
use sage_consensus::MessageEnvelope;
use sage_core::ValidatorId;
use std::collections::BTreeSet;

/// Network transport for sending and receiving consensus messages.
pub trait Transport: Send {
    /// Broadcast a message to all known peers.
    fn broadcast(&mut self, from: ValidatorId, message: MessageEnvelope) -> NetworkResult<()>;

    /// Send a message to a specific peer.
    fn send(
        &mut self,
        from: ValidatorId,
        to: ValidatorId,
        message: MessageEnvelope,
    ) -> NetworkResult<()>;

    /// Receive all pending messages for this validator.
    fn recv(&mut self, validator: ValidatorId) -> NetworkResult<Vec<MessageEnvelope>>;

    /// Return the set of known peer IDs.
    fn peers(&self) -> BTreeSet<ValidatorId>;

    /// Restrict this node to only reach the validators in `keep` (its own side
    /// of a partition), dropping all sends to peers outside the set. Default is
    /// a no-op so transports that do not model partitions are unaffected. The
    /// generic runtime uses this to engage a HEIGHT-TRIGGERED partition mid-run
    /// (the adversarial event at the migration boundary), rather than requiring
    /// a transport-concrete type.
    fn partition_to(&mut self, _keep: &BTreeSet<ValidatorId>) {}

    /// Clear any active partition (network healing). Default no-op.
    fn heal(&mut self) {}

    /// Total messages this transport has actually written to the wire so far.
    /// Default 0 for transports that do not track this (e.g. in-memory). The
    /// real-socket `TcpTransport` overrides it so the multi-process testbed can
    /// sum per-validator wire counts and plot empirical message complexity vs n.
    fn sent_count(&self) -> u64 {
        0
    }
}
