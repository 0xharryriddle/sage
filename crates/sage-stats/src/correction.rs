use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AdjustedDecision {
    pub index: usize,
    pub p_value: f64,
    pub adjusted_alpha: f64,
    pub reject: bool,
}

/// Holm-Bonferroni correction for multiple hypothesis testing.
/// Returns decisions in original input order. A rejection means the null
/// hypothesis can be rejected at the family-wise `alpha` level.
pub fn holm_bonferroni(p_values: &[f64], alpha: f64) -> Vec<AdjustedDecision> {
    let m = p_values.len();
    if m == 0 {
        return vec![];
    }

    // Pair p-values with original indices, sort ascending
    let mut indexed: Vec<(usize, f64)> = p_values.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Step-down: smallest p gets alpha/m, next gets alpha/(m-1), etc.
    let mut rejections = vec![false; m];
    for (rank, &(orig_idx, p)) in indexed.iter().enumerate() {
        let adjusted_alpha = alpha / (m - rank) as f64;
        if p <= adjusted_alpha {
            rejections[orig_idx] = true;
        } else {
            // Once one fails, all larger p-values fail (step-down)
            break;
        }
    }

    p_values
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let rank = indexed.iter().position(|&(idx, _)| idx == i).unwrap_or(0);
            AdjustedDecision {
                index: i,
                p_value: p,
                adjusted_alpha: alpha / (m - rank) as f64,
                reject: rejections[i],
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holm_correction_all_significant() {
        let p_vals = vec![0.001, 0.002, 0.003];
        let decisions = holm_bonferroni(&p_vals, 0.05);
        assert_eq!(decisions.len(), 3);
        for d in &decisions {
            assert!(d.reject);
        }
    }

    #[test]
    fn holm_correction_none_significant() {
        let p_vals = vec![0.5, 0.6, 0.7];
        let decisions = holm_bonferroni(&p_vals, 0.05);
        for d in &decisions {
            assert!(!d.reject);
        }
    }

    #[test]
    fn holm_correction_empty() {
        let decisions = holm_bonferroni(&[], 0.05);
        assert!(decisions.is_empty());
    }
}
