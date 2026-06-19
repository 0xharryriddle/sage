use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SimTime(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DurationMicros(pub u64);

impl SimTime {
    pub const ZERO: Self = Self(0);
    pub fn checked_add(self, duration: DurationMicros) -> Option<Self> {
        self.0.checked_add(duration.0).map(Self)
    }
}
