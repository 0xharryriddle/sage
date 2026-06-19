use crate::block::{Block, Transaction};
use crate::crypto::{encode_bytes, encode_u64, hash_domain, HashDomain};
use crate::{ChainId, ConfigId, CoreError, CoreResult, EngineId, Epoch, Height, StateRoot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainState {
    pub execution: ExecutionState,
    pub platform: PlatformState,
    pub consensus: ConsensusStateEnvelope,
}

impl ChainState {
    pub fn apply_block(&self, block: &Block) -> CoreResult<ChainState> {
        if block.header.chain_id != self.platform.chain_id {
            return Err(CoreError::Encoding(
                "block chain id does not match platform state".to_string(),
            ));
        }
        if block.header.epoch != self.platform.epoch {
            return Err(CoreError::InvalidEpoch {
                expected: self.platform.epoch,
                actual: block.header.epoch,
            });
        }

        let mut next = self.clone();
        for tx in &block.txs {
            next.execution.apply_tx(tx)?;
        }
        let actual = next.root();
        if actual != block.header.state_root {
            return Err(CoreError::StateRootMismatch {
                expected: block.header.state_root,
                actual,
            });
        }
        Ok(next)
    }

    pub fn root(&self) -> StateRoot {
        let mut out = Vec::new();
        self.execution.encode_for_root(&mut out);
        self.platform.encode_for_root(&mut out);
        hash_domain(HashDomain::StateRootV1, &out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionState {
    accounts: BTreeMap<u64, u64>,
}

impl ExecutionState {
    pub fn new_with_accounts(count: u64, balance: u64) -> Self {
        let accounts = (0..count).map(|id| (id, balance)).collect();
        Self { accounts }
    }

    pub fn balance(&self, account: u64) -> u64 {
        self.accounts.get(&account).copied().unwrap_or_default()
    }

    pub fn apply_tx(&mut self, tx: &Transaction) -> CoreResult<()> {
        let from_balance = self.balance(tx.from);
        let debited = from_balance.saturating_sub(tx.amount);
        let credited = self.balance(tx.to).saturating_add(tx.amount);
        self.accounts.insert(tx.from, debited);
        self.accounts.insert(tx.to, credited);
        Ok(())
    }

    fn encode_for_root(&self, out: &mut Vec<u8>) {
        encode_u64(out, self.accounts.len() as u64);
        for (account, balance) in &self.accounts {
            encode_u64(out, *account);
            encode_u64(out, *balance);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformState {
    pub chain_id: ChainId,
    pub epoch: Epoch,
    pub config_id: ConfigId,
    pub migration: Option<MigrationSchedule>,
}

impl PlatformState {
    fn encode_for_root(&self, out: &mut Vec<u8>) {
        encode_bytes(out, self.chain_id.as_str().as_bytes());
        encode_u64(out, self.epoch.get());
        encode_u64(out, self.config_id.get());
        match &self.migration {
            Some(schedule) => {
                out.push(1);
                schedule.encode_for_root(out);
            }
            None => out.push(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationSchedule {
    pub h_d: Height,
    pub h_c: Height,
    pub h_r: Height,
    pub source_engine: EngineId,
    pub target_engine: EngineId,
    pub kappa: u64,
}

impl MigrationSchedule {
    fn encode_for_root(&self, out: &mut Vec<u8>) {
        encode_u64(out, self.h_d.get());
        encode_u64(out, self.h_c.get());
        encode_u64(out, self.h_r.get());
        encode_u64(out, self.source_engine.generation.get());
        encode_u64(out, self.target_engine.generation.get());
        encode_u64(out, self.kappa);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusStateEnvelope {
    pub engine: EngineId,
    pub opaque: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EngineGeneration, EngineId, EngineKind};

    fn state() -> ChainState {
        ChainState {
            execution: ExecutionState::new_with_accounts(4, 100),
            platform: PlatformState {
                chain_id: ChainId::new("sage-test"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                migration: None,
            },
            consensus: ConsensusStateEnvelope {
                engine: EngineId::new(EngineKind::Poa, EngineGeneration::new(1)),
                opaque: vec![],
            },
        }
    }

    #[test]
    fn root_is_deterministic() {
        assert_eq!(state().root(), state().root());
    }

    #[test]
    fn transaction_changes_root() {
        let mut changed = state();
        changed
            .execution
            .apply_tx(&Transaction {
                from: 0,
                to: 1,
                amount: 7,
                nonce: 0,
            })
            .unwrap();
        assert_ne!(state().root(), changed.root());
    }
}
