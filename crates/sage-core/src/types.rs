use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

macro_rules! id_newtype {
    ($name:ident, $inner:ty) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        pub struct $name($inner);

        impl $name {
            pub const fn new(value: $inner) -> Self {
                Self(value)
            }
            pub const fn get(self) -> $inner {
                self.0
            }
        }

        impl From<$inner> for $name {
            fn from(value: $inner) -> Self {
                Self(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ChainId(String);

impl ChainId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ChainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ChainId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

id_newtype!(Epoch, u64);
id_newtype!(ConfigId, u64);
id_newtype!(View, u64);
id_newtype!(Round, u64);
id_newtype!(ValidatorId, u32);
id_newtype!(EngineGeneration, u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Height(u64);

impl Height {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
    pub fn checked_next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
    pub fn checked_prev(self) -> Option<Self> {
        self.0.checked_sub(1).map(Self)
    }
}

impl From<u64> for Height {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Display for Height {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Hash32([u8; 32]);

impl Hash32 {
    pub const ZERO: Self = Self([0; 32]);
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl Default for Hash32 {
    fn default() -> Self {
        Self::ZERO
    }
}

impl fmt::Display for Hash32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

pub type StateRoot = Hash32;
pub type BlockHash = Hash32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EngineKind {
    Poa,
    HotStuff,
    Raft,
    DagBft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EngineId {
    pub kind: EngineKind,
    pub generation: EngineGeneration,
}

impl EngineId {
    pub const fn new(kind: EngineKind, generation: EngineGeneration) -> Self {
        Self { kind, generation }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FinalityTier {
    None,
    Provisional,
    Absolute,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_checked_arithmetic_is_explicit() {
        assert_eq!(Height::new(7).checked_next(), Some(Height::new(8)));
        assert_eq!(Height::new(0).checked_prev(), None);
    }

    #[test]
    fn engine_id_equality_includes_generation() {
        let a = EngineId::new(EngineKind::Poa, EngineGeneration::new(1));
        let b = EngineId::new(EngineKind::Poa, EngineGeneration::new(2));
        assert_ne!(a, b);
    }
}
