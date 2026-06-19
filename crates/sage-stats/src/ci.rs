use crate::error::{StatsError, StatsResult};
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Interval {
    pub lower: f64,
    pub upper: f64,
    pub confidence: f64,
}

/// Bootstrap confidence interval for the mean.
/// Resamples `values` with replacement `samples` times using seeded RNG.
pub fn mean_ci_bootstrap(
    values: &[f64],
    confidence: f64,
    seed: u64,
    samples: usize,
) -> StatsResult<Interval> {
    if values.is_empty() {
        return Err(StatsError::EmptyInput);
    }
    if confidence <= 0.0 || confidence >= 1.0 {
        return Err(StatsError::InvalidConfidence(confidence));
    }
    let n = values.len();
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut means = Vec::with_capacity(samples);
    for _ in 0..samples {
        let mut sum = 0.0;
        for _ in 0..n {
            let idx = rng.gen_range(0..n);
            sum += values[idx];
        }
        means.push(sum / n as f64);
    }
    means.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let tail = (1.0 - confidence) / 2.0;
    let lower_idx = ((tail * samples as f64) as usize).min(samples - 1);
    let upper_idx = (((1.0 - tail) * samples as f64) as usize).min(samples - 1);
    Ok(Interval {
        lower: means[lower_idx],
        upper: means[upper_idx],
        confidence,
    })
}

/// Wilson score interval for a proportion.
pub fn wilson_interval(successes: u64, trials: u64, confidence: f64) -> StatsResult<Interval> {
    if trials == 0 {
        return Err(StatsError::InvalidTrials(trials));
    }
    if confidence <= 0.0 || confidence >= 1.0 {
        return Err(StatsError::InvalidConfidence(confidence));
    }
    let p = successes as f64 / trials as f64;
    let z = norm_quantile(1.0 - (1.0 - confidence) / 2.0);
    let denom = 1.0 + z * z / trials as f64;
    let centre = (p + z * z / (2.0 * trials as f64)) / denom;
    let se_sq = p * (1.0 - p) / trials as f64 + z * z / (4.0 * trials as f64 * trials as f64);
    let margin = z / denom * se_sq.sqrt();
    Ok(Interval {
        lower: (centre - margin).max(0.0),
        upper: (centre + margin).min(1.0),
        confidence,
    })
}

/// Simple normal quantile approximation (Abramowitz and Stegun 26.2.23).
fn norm_quantile(p: f64) -> f64 {
    let p = p.clamp(0.000001, 0.999999);
    let t = (-2.0 * (1.0 - p).ln()).sqrt();
    let c0 = 2.515517;
    let c1 = 0.802853;
    let c2 = 0.010328;
    let d1 = 1.432788;
    let d2 = 0.189269;
    let d3 = 0.001308;
    t - (c0 + c1 * t + c2 * t * t) / (1.0 + d1 * t + d2 * t * t + d3 * t * t * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_ci_for_constant_values_is_tight() {
        let vals = vec![5.0; 100];
        let ci = mean_ci_bootstrap(&vals, 0.95, 42, 1000).unwrap();
        assert!(ci.lower <= 5.0 && ci.upper >= 5.0);
        assert!((ci.upper - ci.lower) < 0.01);
    }

    #[test]
    fn wilson_ci_for_zero_successes_is_bounded() {
        let ci = wilson_interval(0, 30, 0.95).unwrap();
        assert!(ci.lower >= 0.0);
        assert!(ci.upper <= 1.0);
    }

    #[test]
    fn wilson_ci_for_all_successes_is_bounded() {
        let ci = wilson_interval(30, 30, 0.95).unwrap();
        assert!(ci.lower <= 1.0);
        assert!(ci.upper <= 1.0);
    }
}
