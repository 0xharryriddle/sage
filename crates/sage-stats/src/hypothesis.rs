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
}
