use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewEnvironmentInfo {
    pub pr_number: u64,
    pub preview_url: String,
    pub age_hours: u64,
    pub is_pr_closed: bool,
}

pub struct PreviewReaperEngine;

impl PreviewReaperEngine {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic evaluation of expired or closed PR preview environments (TTL > 48h or PR closed)
    pub fn sweep_stale_previews(&self, previews: &[PreviewEnvironmentInfo]) -> Vec<u64> {
        let mut reaped_pr_numbers = Vec::new();

        for preview in previews {
            if preview.is_pr_closed || preview.age_hours > 48 {
                reaped_pr_numbers.push(preview.pr_number);
            }
        }

        reaped_pr_numbers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reaps_closed_or_expired_previews() {
        let engine = PreviewReaperEngine::new();
        let previews = vec![
            PreviewEnvironmentInfo {
                pr_number: 101,
                preview_url: "https://pr-101.preview.oyatie.internal".to_string(),
                age_hours: 12,
                is_pr_closed: true, // Reaped!
            },
            PreviewEnvironmentInfo {
                pr_number: 102,
                preview_url: "https://pr-102.preview.oyatie.internal".to_string(),
                age_hours: 55, // Expired (>48h)!
                is_pr_closed: false,
            },
            PreviewEnvironmentInfo {
                pr_number: 103,
                preview_url: "https://pr-103.preview.oyatie.internal".to_string(),
                age_hours: 4,
                is_pr_closed: false, // Active!
            },
        ];

        let reaped = engine.sweep_stale_previews(&previews);
        assert_eq!(reaped, vec![101, 102]);
    }
}
