//! In-memory message-passing transport using VecDeque per validator.
use crate::error::{NetworkError, NetworkResult};
use crate::transport::Transport;
use sage_consensus::MessageEnvelope;
use sage_core::ValidatorId;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone)]
pub struct InMemoryTransport {
    queues: BTreeMap<ValidatorId, VecDeque<MessageEnvelope>>,
    peer_ids: BTreeSet<ValidatorId>,
    /// Optional partition: when set, a message is delivered only if sender and
    /// recipient fall in the same side. This models a real network split, where
    /// each isolated group can only hear itself. Cleared by `heal`.
    partition: Option<(BTreeSet<ValidatorId>, BTreeSet<ValidatorId>)>,
}

impl InMemoryTransport {
    pub fn new(validator_ids: impl IntoIterator<Item = ValidatorId>) -> Self {
        let peer_ids: BTreeSet<ValidatorId> = validator_ids.into_iter().collect();
        let queues = peer_ids.iter().map(|&id| (id, VecDeque::new())).collect();
        Self {
            queues,
            peer_ids,
            partition: None,
        }
    }

    /// Install a two-sided partition. Messages only flow within a side.
    pub fn partition(&mut self, side_a: BTreeSet<ValidatorId>, side_b: BTreeSet<ValidatorId>) {
        self.partition = Some((side_a, side_b));
    }

    /// Remove any active partition (network healing).
    pub fn heal(&mut self) {
        self.partition = None;
    }

    /// True iff `from` and `to` can currently exchange messages.
    fn same_side(&self, from: ValidatorId, to: ValidatorId) -> bool {
        match &self.partition {
            None => true,
            Some((a, b)) => {
                (a.contains(&from) && a.contains(&to)) || (b.contains(&from) && b.contains(&to))
            }
        }
    }
}

impl Transport for InMemoryTransport {
    fn broadcast(&mut self, from: ValidatorId, message: MessageEnvelope) -> NetworkResult<()> {
        let peers: Vec<ValidatorId> = self.peer_ids.iter().copied().collect();
        for id in peers {
            if id != from {
                self.send(from, id, message.clone())?;
            }
        }
        Ok(())
    }

    fn send(
        &mut self,
        from: ValidatorId,
        to: ValidatorId,
        message: MessageEnvelope,
    ) -> NetworkResult<()> {
        // A partition silently drops cross-side messages, exactly like a real
        // network split. The send still returns Ok so callers behave normally.
        if !self.same_side(from, to) {
            return Ok(());
        }
        let q = self
            .queues
            .get_mut(&to)
            .ok_or(NetworkError::UnknownPeer(to))?;
        q.push_back(message);
        Ok(())
    }

    fn recv(&mut self, validator: ValidatorId) -> NetworkResult<Vec<MessageEnvelope>> {
        let q = self
            .queues
            .get_mut(&validator)
            .ok_or(NetworkError::UnknownPeer(validator))?;
        let msgs: Vec<MessageEnvelope> = q.drain(..).collect();
        Ok(msgs)
    }

    fn peers(&self) -> BTreeSet<ValidatorId> {
        self.peer_ids.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_consensus::{ConsensusMessage, HotStuffMessage};
    use sage_core::{
        Block, BlockHash, BlockHeader, ChainId, ConfigId, EngineGeneration, EngineId, EngineKind,
        Epoch, FinalityTier, Height, StateRoot, View,
    };

    fn dummy_envelope(from: ValidatorId, to: Option<ValidatorId>) -> MessageEnvelope {
        let block = Block {
            header: BlockHeader {
                chain_id: ChainId::new("test"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                height: Height::new(1),
                parent_hash: BlockHash::new([0; 32]),
                state_root: StateRoot::new([0; 32]),
                engine_id: EngineId::new(EngineKind::Poa, EngineGeneration::new(1)),
                finality_tier: FinalityTier::None,
                manifest_hash: None,
            },
            txs: vec![],
        };
        MessageEnvelope {
            from,
            to,
            chain_id: ChainId::new("test"),
            epoch: Epoch::new(1),
            config_id: ConfigId::new(1),
            message: ConsensusMessage::HotStuff(HotStuffMessage::Vote {
                view: View::new(0),
                block,
                block_hash: BlockHash::new([1; 32]),
            }),
        }
    }

    #[test]
    fn broadcast_reaches_all_peers() {
        let ids = (0..4).map(ValidatorId::new).collect::<Vec<_>>();
        let mut net = InMemoryTransport::new(ids.iter().copied());

        let msg = dummy_envelope(ValidatorId::new(0), None);
        net.broadcast(ValidatorId::new(0), msg).unwrap();

        let msgs_0 = net.recv(ValidatorId::new(0)).unwrap();
        assert!(msgs_0.is_empty());

        for i in 1..4u32 {
            let msgs = net.recv(ValidatorId::new(i)).unwrap();
            assert_eq!(msgs.len(), 1);
        }
    }

    #[test]
    fn recv_drains_queue() {
        let ids = (0..2).map(ValidatorId::new).collect::<Vec<_>>();
        let mut net = InMemoryTransport::new(ids.iter().copied());

        let msg = dummy_envelope(ValidatorId::new(1), Some(ValidatorId::new(0)));
        net.send(ValidatorId::new(1), ValidatorId::new(0), msg.clone())
            .unwrap();
        net.send(ValidatorId::new(1), ValidatorId::new(0), msg)
            .unwrap();

        let msgs = net.recv(ValidatorId::new(0)).unwrap();
        assert_eq!(msgs.len(), 2);

        let empty = net.recv(ValidatorId::new(0)).unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn unknown_peer_errors() {
        let ids = [ValidatorId::new(0)];
        let mut net = InMemoryTransport::new(ids.iter().copied());

        let msg = dummy_envelope(ValidatorId::new(0), Some(ValidatorId::new(99)));
        let result = net.send(ValidatorId::new(0), ValidatorId::new(99), msg);
        assert!(result.is_err());
    }

    #[test]
    fn partition_isolates_sides_and_heals() {
        let ids = (0..4).map(ValidatorId::new).collect::<Vec<_>>();
        let mut net = InMemoryTransport::new(ids.iter().copied());
        let side_a: BTreeSet<ValidatorId> = [ValidatorId::new(0), ValidatorId::new(1)].into();
        let side_b: BTreeSet<ValidatorId> = [ValidatorId::new(2), ValidatorId::new(3)].into();
        net.partition(side_a, side_b);

        // Cross-side send (0 -> 2) is dropped.
        net.send(
            ValidatorId::new(0),
            ValidatorId::new(2),
            dummy_envelope(ValidatorId::new(0), Some(ValidatorId::new(2))),
        )
        .unwrap();
        assert!(net.recv(ValidatorId::new(2)).unwrap().is_empty());

        // Same-side send (0 -> 1) is delivered.
        net.send(
            ValidatorId::new(0),
            ValidatorId::new(1),
            dummy_envelope(ValidatorId::new(0), Some(ValidatorId::new(1))),
        )
        .unwrap();
        assert_eq!(net.recv(ValidatorId::new(1)).unwrap().len(), 1);

        // Healing restores cross-side delivery.
        net.heal();
        net.send(
            ValidatorId::new(0),
            ValidatorId::new(2),
            dummy_envelope(ValidatorId::new(0), Some(ValidatorId::new(2))),
        )
        .unwrap();
        assert_eq!(net.recv(ValidatorId::new(2)).unwrap().len(), 1);
    }
}
