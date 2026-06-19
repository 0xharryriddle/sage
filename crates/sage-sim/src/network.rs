use crate::adversary::PartitionState;
use crate::{Event, EventKind};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sage_consensus::MessageEnvelope;
use sage_core::{DurationMicros, SimTime, ValidatorId};

#[derive(Debug, Clone)]
pub struct NetworkModel {
    rng: ChaCha20Rng,
    mean_delay_micros: u64,
    next_seq: u64,
}

impl NetworkModel {
    pub fn new(seed: u64, mean_delay_micros: u64) -> Self {
        Self {
            rng: ChaCha20Rng::seed_from_u64(seed),
            mean_delay_micros,
            next_seq: 0,
        }
    }

    pub fn try_send(
        &mut self,
        from: ValidatorId,
        to: ValidatorId,
        message: MessageEnvelope,
        now: SimTime,
        partition: &PartitionState,
    ) -> Option<Event> {
        if !partition.same_side(from, to) {
            return None;
        }
        let max_delay = self.mean_delay_micros.saturating_mul(2).max(1);
        let delay = self.rng.gen_range(1..=max_delay);
        let at = now.checked_add(DurationMicros(delay)).unwrap_or(now);
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        Some(Event {
            at,
            seq,
            kind: EventKind::DeliverMessage {
                to,
                message: Box::new(message),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_core::{
        Block, BlockHeader, ChainId, ConfigId, EngineGeneration, EngineId, EngineKind, Epoch,
        FinalityTier, Hash32,
    };

    #[test]
    fn drops_cross_partition_message() {
        let mut network = NetworkModel::new(1, 10);
        let mut partition = PartitionState::inactive();
        partition.start(4, 2);
        let block = Block {
            header: BlockHeader {
                chain_id: ChainId::new("sage"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                height: sage_core::Height::new(0),
                parent_hash: Hash32::ZERO,
                state_root: Hash32::ZERO,
                engine_id: EngineId::new(EngineKind::Poa, EngineGeneration::new(1)),
                finality_tier: FinalityTier::Absolute,
                manifest_hash: None,
            },
            txs: vec![],
        };
        let msg = MessageEnvelope {
            from: ValidatorId::new(0),
            to: None,
            chain_id: ChainId::new("sage"),
            epoch: Epoch::new(1),
            config_id: ConfigId::new(1),
            message: sage_consensus::ConsensusMessage::Poa(sage_consensus::PoaMessage::Vote {
                block_hash: block.hash(),
                block,
            }),
        };
        assert!(network
            .try_send(
                ValidatorId::new(0),
                ValidatorId::new(1),
                msg.clone(),
                SimTime::ZERO,
                &partition
            )
            .is_some());
        assert!(network
            .try_send(
                ValidatorId::new(0),
                ValidatorId::new(2),
                msg,
                SimTime::ZERO,
                &partition
            )
            .is_none());
    }
}
