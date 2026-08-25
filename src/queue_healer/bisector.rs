use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BisectionResult {
    pub culprit_pr: Option<u64>,
    pub clean_prs: Vec<u64>,
    pub iterations_performed: usize,
    pub diagnosis: String,
}

pub struct MergeTrainBisector;

impl Default for MergeTrainBisector {
    fn default() -> Self {
        Self::new()
    }
}

impl MergeTrainBisector {
    pub fn new() -> Self {
        Self
    }

    /// Performs binary search bisection across a batch of speculative merge PRs
    /// given a test evaluator closure `test_batch(slice) -> bool`
    pub fn bisect_batch<F>(&self, pr_batch: &[u64], mut test_batch: F) -> Result<BisectionResult>
    where
        F: FnMut(&[u64]) -> bool,
    {
        if pr_batch.is_empty() {
            return Ok(BisectionResult {
                culprit_pr: None,
                clean_prs: Vec::new(),
                iterations_performed: 0,
                diagnosis: "Batch is empty".to_string(),
            });
        }

        // If the entire batch passes, no culprit exists
        if test_batch(pr_batch) {
            return Ok(BisectionResult {
                culprit_pr: None,
                clean_prs: pr_batch.to_vec(),
                iterations_performed: 1,
                diagnosis: "Entire batch passed integration verification".to_string(),
            });
        }

        // Single PR batch that fails is the culprit
        if pr_batch.len() == 1 {
            return Ok(BisectionResult {
                culprit_pr: Some(pr_batch[0]),
                clean_prs: Vec::new(),
                iterations_performed: 1,
                diagnosis: format!("PR #{} failed integration in isolation", pr_batch[0]),
            });
        }

        let mut low = 0;
        let mut high = pr_batch.len() - 1;
        let mut iterations = 0;
        let mut culprit = None;

        while low <= high {
            iterations += 1;
            if low == high {
                culprit = Some(pr_batch[low]);
                break;
            }

            let mid = low + (high - low) / 2;
            let left_sub_batch = &pr_batch[low..=mid];

            info!(
                "Bisection iteration {}: testing sub-batch {:?}",
                iterations, left_sub_batch
            );

            let left_ok = test_batch(left_sub_batch);
            if !left_ok {
                // Failure is in the left half
                high = mid;
            } else {
                // Left half passed, failure must be in right half
                low = mid + 1;
            }
        }

        let clean_prs: Vec<u64> = pr_batch
            .iter()
            .copied()
            .filter(|&pr| Some(pr) != culprit)
            .collect();

        let diagnosis = if let Some(c) = culprit {
            format!(
                "Isolated regression to PR #{} after {} bisection step(s). Remaining {} PR(s) validated clean.",
                c,
                iterations,
                clean_prs.len()
            )
        } else {
            "Could not isolate single culprit; multi-PR incompatibility detected".to_string()
        };

        Ok(BisectionResult {
            culprit_pr: culprit,
            clean_prs,
            iterations_performed: iterations,
            diagnosis,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bisector_isolates_culprit() {
        let bisector = MergeTrainBisector::new();
        let batch = vec![2137, 2136, 2135, 2130];
        let culprit_target = 2135;

        let res = bisector
            .bisect_batch(&batch, |slice| {
                // Returns false if the slice contains the culprit PR
                !slice.contains(&culprit_target)
            })
            .expect("Bisection succeeds");

        assert_eq!(res.culprit_pr, Some(2135));
        assert_eq!(res.clean_prs, vec![2137, 2136, 2130]);
        assert!(res.iterations_performed <= 3);
    }
}
