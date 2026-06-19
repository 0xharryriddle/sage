use crate::config::WorkloadConfig;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sage_core::{Height, Transaction};

#[derive(Debug, Clone)]
pub struct WorkloadGenerator {
    rng: ChaCha20Rng,
    cfg: WorkloadConfig,
}

impl WorkloadGenerator {
    pub fn new(seed: u64, cfg: WorkloadConfig) -> Self {
        Self {
            rng: ChaCha20Rng::seed_from_u64(seed),
            cfg,
        }
    }

    pub fn next_batch(&mut self, height: Height) -> Vec<Transaction> {
        (0..self.cfg.txs_per_block)
            .map(|idx| {
                let from = self.rng.gen_range(0..self.cfg.state_accounts);
                let mut to = self.rng.gen_range(0..self.cfg.state_accounts);
                if to == from {
                    to = (to + 1) % self.cfg.state_accounts;
                }
                Transaction {
                    from,
                    to,
                    amount: 1,
                    nonce: height
                        .get()
                        .saturating_mul(1_000_000)
                        .saturating_add(idx as u64),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_seed_produces_same_batch() {
        let cfg = WorkloadConfig::default();
        let mut a = WorkloadGenerator::new(7, cfg.clone());
        let mut b = WorkloadGenerator::new(7, cfg);
        assert_eq!(a.next_batch(Height::new(1)), b.next_batch(Height::new(1)));
    }
}
