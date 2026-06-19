//! Shared in-memory transport: a clonable handle over one shared message bus.
//!
//! Unlike [`InMemoryTransport`](crate::InMemoryTransport), whose queues live
//! inside the single value, this transport keeps every validator's inbound
//! queue behind a shared `Arc<Mutex<..>>`. Each validator holds its OWN
//! [`SharedMemoryTransport`] handle (with its own `local_id`) but they all
//! read/write the same bus. That lets several [`ProcessValidator`]s run in one
//! test process with no shared node memory — each only touches the bus through
//! the `Transport` trait — exercising the exact multi-process driver path
//! without binding real sockets. The TCP transport is the production path; this
//! is the fast, deterministic test double for the same topology.

use crate::error::{NetworkError, NetworkResult};
use crate::transport::Transport;
use sage_consensus::MessageEnvelope;
use sage_core::ValidatorId;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

type Bus = Arc<Mutex<BTreeMap<ValidatorId, VecDeque<MessageEnvelope>>>>;

/// One validator's handle onto a shared in-memory message bus.
#[derive(Clone)]
pub struct SharedMemoryTransport {
    local_id: ValidatorId,
    peer_ids: BTreeSet<ValidatorId>,
    bus: Bus,
    /// Peers this handle currently refuses to send to (its blocked set under a
    /// partition). Mirrors `TcpTransport`'s `blocked` so the in-memory test
    /// double exercises the identical partition semantics as the real socket
    /// path: cross-side sends are silently dropped at `send`.
    blocked: BTreeSet<ValidatorId>,
}

impl SharedMemoryTransport {
    /// Build one handle per validator id, all sharing a single bus. Returns the
    /// handles in id order; hand each to a separate `ProcessValidator`.
    pub fn mesh(ids: &[ValidatorId]) -> Vec<SharedMemoryTransport> {
        let peer_ids: BTreeSet<ValidatorId> = ids.iter().copied().collect();
        let bus: Bus = Arc::new(Mutex::new(
            peer_ids.iter().map(|&id| (id, VecDeque::new())).collect(),
        ));
        ids.iter()
            .map(|&id| SharedMemoryTransport {
                local_id: id,
                peer_ids: peer_ids.clone(),
                bus: Arc::clone(&bus),
                blocked: BTreeSet::new(),
            })
            .collect()
    }
}

impl Transport for SharedMemoryTransport {
    fn broadcast(&mut self, from: ValidatorId, message: MessageEnvelope) -> NetworkResult<()> {
        let peers: Vec<ValidatorId> = self.peer_ids.iter().copied().collect();
        for id in peers {
            if id != from && id != self.local_id {
                self.send(from, id, message.clone())?;
            }
        }
        Ok(())
    }

    fn send(
        &mut self,
        _from: ValidatorId,
        to: ValidatorId,
        message: MessageEnvelope,
    ) -> NetworkResult<()> {
        // A blocked peer silently drops, exactly like a partitioned socket.
        if self.blocked.contains(&to) {
            return Ok(());
        }
        let mut bus = self
            .bus
            .lock()
            .map_err(|_| NetworkError::Io("shared bus poisoned".to_string()))?;
        let q = bus.get_mut(&to).ok_or(NetworkError::UnknownPeer(to))?;
        q.push_back(message);
        Ok(())
    }

    fn recv(&mut self, validator: ValidatorId) -> NetworkResult<Vec<MessageEnvelope>> {
        let mut bus = self
            .bus
            .lock()
            .map_err(|_| NetworkError::Io("shared bus poisoned".to_string()))?;
        let q = bus
            .get_mut(&validator)
            .ok_or(NetworkError::UnknownPeer(validator))?;
        Ok(q.drain(..).collect())
    }

    fn peers(&self) -> BTreeSet<ValidatorId> {
        self.peer_ids.clone()
    }

    fn partition_to(&mut self, keep: &BTreeSet<ValidatorId>) {
        for id in self.peer_ids.iter().copied().collect::<Vec<_>>() {
            if id == self.local_id {
                continue;
            }
            if keep.contains(&id) {
                self.blocked.remove(&id);
            } else {
                self.blocked.insert(id);
            }
        }
    }

    fn heal(&mut self) {
        self.blocked.clear();
    }
}
