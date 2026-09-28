use crate::error::StatsResult;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MannWhitneyResult {
    pub u_statistic: f64,
    pub z_score: f64,
    pub p_value: f64,
    pub rank_biserial: f64,
}

/// Mann-Whitney U test (two-sided, normal approximation).
pub fn mann_whitney_u(a: &[f64], b: &[f64]) -> StatsResult<MannWhitneyResult> {
    let n1 = a.len() as f64;
    let n2 = b.len() as f64;
    // Rank all values
    let mut indexed: Vec<(f64, bool)> = a.iter().map(|v| (*v, true)).collect();
    indexed.extend(b.iter().map(|v| (*v, false)));
    indexed.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));

    let n = indexed.len();
    let mut ranks = vec![0.0; n];
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && (indexed[j].0 - indexed[i].0).abs() < f64::EPSILON {
            j += 1;
        }
        let tie_count = (j - i) as f64;
        let rank_sum = ((i + 1) as f64 + j as f64) * tie_count / 2.0;
        let rank_val = rank_sum / tie_count;
        for item in ranks.iter_mut().take(j).skip(i) {
            *item = rank_val;
        }
        i = j;
    }

    let mut r1 = 0.0;
    let mut count_a = 0.0;
    for k in 0..n {
        if indexed[k].1 {
            r1 += ranks[k];
            count_a += 1.0;
        }
    }

    let u1 = r1 - count_a * (count_a + 1.0) / 2.0;
    let u2 = n1 * n2 - u1;
    let u = u1.max(u2);

    let mean_u = n1 * n2 / 2.0;
    let std_u = (n1 * n2 * (n1 + n2 + 1.0) / 12.0).sqrt();
    let z = if std_u > 0.0 {
        (u - mean_u) / std_u
    } else {
        0.0
    };

    // Two-sided p-value from normal approximation
    let p_value = 2.0 * (1.0 - normal_cdf(z.abs()));

    // Rank-biserial correlation
    let rank_biserial = if n1 * n2 > 0.0 {
        1.0 - (2.0 * u1) / (n1 * n2)
    } else {
        0.0
    };

    Ok(MannWhitneyResult {
        u_statistic: u,
        z_score: z,
        p_value,
        rank_biserial,
    })
}

/// Outcome of a paired two-sided test over seed-matched observations.
///
/// Use paired tests only when observations were matched by design (for
/// example, the same seed and workload in both arms). This function does not
/// establish that any caller's experiment satisfies that precondition.
#[derive(Debug, Clone, Serialize)]
pub struct PairedResult {
    /// Number of usable pairs.
    pub n_pairs: usize,
    /// Pairs whose difference is exactly zero; excluded from the signed-rank
    /// statistic by Wilcoxon's original zero-discarding rule.
    pub n_zero_differences: usize,
    /// Median of the per-pair differences `a[i] - b[i]`.
    pub median_difference: f64,
    /// Wilcoxon signed-rank statistic (smaller of the two rank sums).
    pub w_statistic: f64,
    /// Two-sided p-value from the signed-rank normal approximation with a tie
    /// correction applied to the null variance.
    pub wilcoxon_p_value: f64,
    /// Two-sided p-value from an exact sign-flip permutation test when the pair
    /// count is small enough to enumerate, otherwise `None`.
    pub exact_permutation_p_value: Option<f64>,
    /// Matched-pairs rank-biserial effect size in `[-1, 1]`.
    pub rank_biserial: f64,
}

/// Wilcoxon signed-rank test over seed-matched pairs (two-sided).
///
/// Returns `EmptyInput` when the inputs differ in length or hold no pairs.
/// When every difference is zero the test is undefined in the usual sense; we
/// report `p = 1.0` with a zero effect size, which is the correct
/// "indistinguishable" verdict for two arms that tied on every seed.
pub fn wilcoxon_signed_rank(a: &[f64], b: &[f64]) -> StatsResult<PairedResult> {
    if a.len() != b.len() || a.is_empty() {
        return Err(crate::error::StatsError::EmptyInput);
    }

    let diffs: Vec<f64> = a.iter().zip(b.iter()).map(|(x, y)| x - y).collect();
    let n_pairs = diffs.len();
    let nonzero: Vec<f64> = diffs.iter().copied().filter(|d| *d != 0.0).collect();
    let n_zero_differences = n_pairs - nonzero.len();
    let median_difference = median(&diffs);

    if nonzero.is_empty() {
        return Ok(PairedResult {
            n_pairs,
            n_zero_differences,
            median_difference,
            w_statistic: 0.0,
            wilcoxon_p_value: 1.0,
            exact_permutation_p_value: Some(1.0),
            rank_biserial: 0.0,
        });
    }

    // Rank absolute differences, averaging ranks within tie groups.
    let mut absolute: Vec<(f64, f64)> = nonzero.iter().map(|d| (d.abs(), *d)).collect();
    absolute.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    let m = absolute.len();
    let mut ranks = vec![0.0f64; m];
    let mut tie_correction = 0.0f64;
    let mut i = 0;
    while i < m {
        let mut j = i + 1;
        while j < m && (absolute[j].0 - absolute[i].0).abs() < f64::EPSILON {
            j += 1;
        }
        let tie_count = (j - i) as f64;
        let averaged = ((i + 1) as f64 + j as f64) / 2.0;
        for slot in ranks.iter_mut().take(j).skip(i) {
            *slot = averaged;
        }
        tie_correction += tie_count * tie_count * tie_count - tie_count;
        i = j;
    }

    let mut w_positive = 0.0f64;
    let mut w_negative = 0.0f64;
    for (idx, (_, signed)) in absolute.iter().enumerate() {
        if *signed > 0.0 {
            w_positive += ranks[idx];
        } else {
            w_negative += ranks[idx];
        }
    }
    let w_statistic = w_positive.min(w_negative);

    let mf = m as f64;
    let mean_w = mf * (mf + 1.0) / 4.0;
    let variance = (mf * (mf + 1.0) * (2.0 * mf + 1.0) - tie_correction / 2.0) / 24.0;
    let wilcoxon_p_value = if variance > 0.0 {
        let z = (w_statistic - mean_w) / variance.sqrt();
        (2.0 * (1.0 - normal_cdf(z.abs()))).clamp(0.0, 1.0)
    } else {
        1.0
    };

    // Matched-pairs rank-biserial: normalized difference of signed rank sums.
    let total_rank = mf * (mf + 1.0) / 2.0;
    let rank_biserial = if total_rank > 0.0 {
        (w_positive - w_negative) / total_rank
    } else {
        0.0
    };

    let exact_permutation_p_value = exact_sign_flip_p_value(&nonzero);

    Ok(PairedResult {
        n_pairs,
        n_zero_differences,
        median_difference,
        w_statistic,
        wilcoxon_p_value,
        exact_permutation_p_value,
        rank_biserial,
    })
}

/// Exact two-sided sign-flip permutation p-value on the mean difference.
///
/// Enumerates all `2^m` sign assignments, so it is only attempted for
/// `m <= 20` (about one million evaluations). Larger inputs return `None` and
/// callers should rely on the signed-rank approximation instead.
fn exact_sign_flip_p_value(nonzero_diffs: &[f64]) -> Option<f64> {
    const MAX_EXACT_PAIRS: usize = 20;
    let m = nonzero_diffs.len();
    if m == 0 || m > MAX_EXACT_PAIRS {
        return None;
    }
    let observed: f64 = nonzero_diffs.iter().sum::<f64>().abs();
    let total = 1usize << m;
    let mut at_least_as_extreme = 0usize;
    for mask in 0..total {
        let mut sum = 0.0f64;
        for (bit, diff) in nonzero_diffs.iter().enumerate() {
            if mask & (1 << bit) == 0 {
                sum += *diff;
            } else {
                sum -= *diff;
            }
        }
        // Tolerance guards against float drift making an identical
        // reassignment look strictly smaller than the observed statistic.
        if sum.abs() >= observed - 1e-12 {
            at_least_as_extreme += 1;
        }
    }
    Some((at_least_as_extreme as f64 / total as f64).clamp(0.0, 1.0))
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Standard normal CDF approximation.
fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

/// Error function approximation (Abramowitz and Stegun 7.1.26).
fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let p = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - ((((a5 * t + a4) * t + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    sign * y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mann_whitney_different_distributions() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![6.0, 7.0, 8.0, 9.0, 10.0];
        let result = mann_whitney_u(&a, &b).unwrap();
        assert!(result.p_value < 0.05);
    }

    #[test]
    fn mann_whitney_same_distribution() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![1.1, 2.1, 3.1, 4.1, 5.1];
        let result = mann_whitney_u(&a, &b).unwrap();
        assert!(result.p_value > 0.05);
    }

    #[test]
    fn wilcoxon_rejects_length_mismatch() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0];
        assert!(wilcoxon_signed_rank(&a, &b).is_err());
    }

    #[test]
    fn wilcoxon_all_zero_differences_is_indistinguishable() {
        // Two arms that tie on every seed must NOT report significance. This is
        // the SAGE-vs-CoxStyle downtime case.
        let a = vec![122.0; 12];
        let b = vec![122.0; 12];
        let result = wilcoxon_signed_rank(&a, &b).unwrap();
        assert_eq!(result.n_pairs, 12);
        assert_eq!(result.n_zero_differences, 12);
        assert_eq!(result.median_difference, 0.0);
        assert_eq!(result.wilcoxon_p_value, 1.0);
        assert_eq!(result.rank_biserial, 0.0);
    }

    #[test]
    fn wilcoxon_detects_consistent_paired_shift() {
        // Every pair moves the same direction: the paired test must reject even
        // though the magnitude is small relative to the spread.
        let a: Vec<f64> = (0..15).map(|i| i as f64).collect();
        let b: Vec<f64> = (0..15).map(|i| i as f64 + 7.0).collect();
        let result = wilcoxon_signed_rank(&a, &b).unwrap();
        assert_eq!(result.n_pairs, 15);
        assert_eq!(result.n_zero_differences, 0);
        assert_eq!(result.median_difference, -7.0);
        assert!(result.wilcoxon_p_value < 0.01);
        // Wholly negative differences drive the effect size to its lower bound.
        assert!((result.rank_biserial + 1.0).abs() < 1e-9);
    }

    #[test]
    fn wilcoxon_exact_permutation_matches_hand_computation() {
        // m = 3 nonzero differences, all positive => only the all-same-sign
        // assignment is at least as extreme in each tail, so p = 2/8 = 0.25.
        let a = vec![2.0, 4.0, 6.0];
        let b = vec![1.0, 1.0, 1.0];
        let result = wilcoxon_signed_rank(&a, &b).unwrap();
        let exact = result
            .exact_permutation_p_value
            .expect("3 pairs is inside the exact-enumeration bound");
        assert!((exact - 0.25).abs() < 1e-12, "got {exact}");
    }

    #[test]
    fn wilcoxon_skips_exact_test_beyond_enumeration_bound() {
        // 21 nonzero pairs exceeds MAX_EXACT_PAIRS: the approximation stands
        // alone rather than silently enumerating 2^21 sign assignments.
        let a: Vec<f64> = (1..=21).map(|i| i as f64).collect();
        let b: Vec<f64> = (1..=21).map(|i| i as f64 * 2.0).collect();
        let result = wilcoxon_signed_rank(&a, &b).unwrap();
        assert_eq!(result.n_pairs, 21);
        assert!(result.exact_permutation_p_value.is_none());
    }

    #[test]
    fn wilcoxon_tolerates_ties_among_nonzero_differences() {
        // Repeated absolute differences exercise the tie correction; the test
        // must still return a usable probability rather than NaN.
        let a = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0];
        let b = vec![13.0, 23.0, 33.0, 43.0, 53.0, 63.0];
        let result = wilcoxon_signed_rank(&a, &b).unwrap();
        assert_eq!(result.median_difference, -3.0);
        assert!(result.wilcoxon_p_value.is_finite());
        assert!(result.wilcoxon_p_value <= 1.0);
    }
}
