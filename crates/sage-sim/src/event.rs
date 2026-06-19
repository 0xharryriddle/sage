use sage_consensus::MessageEnvelope;
use sage_core::{Height, SimTime, ValidatorId};
use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub enum EventKind {
    ProposeHeight {
        height: Height,
    },
    DeliverMessage {
        to: ValidatorId,
        message: Box<MessageEnvelope>,
    },
    TryFinalize {
        validator: ValidatorId,
    },
}

#[derive(Debug, Clone)]
pub struct Event {
    pub at: SimTime,
    pub seq: u64,
    pub kind: EventKind,
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.at == other.at && self.seq == other.seq
    }
}
impl Eq for Event {}
impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .at
            .cmp(&self.at)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BinaryHeap;
    #[test]
    fn event_queue_pops_earliest_time_then_sequence() {
        let mut heap = BinaryHeap::new();
        heap.push(Event {
            at: SimTime(10),
            seq: 0,
            kind: EventKind::ProposeHeight {
                height: Height::new(1),
            },
        });
        heap.push(Event {
            at: SimTime(5),
            seq: 1,
            kind: EventKind::ProposeHeight {
                height: Height::new(1),
            },
        });
        assert_eq!(heap.pop().unwrap().at, SimTime(5));
    }
}
