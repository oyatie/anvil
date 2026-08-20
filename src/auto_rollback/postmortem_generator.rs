#[derive(Clone, Debug)]
pub struct PostmortemBundle {
    pub service_name: String,
    pub root_cause: String,
    pub incident_timeline: Vec<String>,
    pub action_items: Vec<String>,
    pub markdown_report: String,
}

#[derive(Clone, Debug, Default)]
pub struct PostmortemGenerator;

impl PostmortemGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_postmortem(
        &self,
        service: &str,
        root_cause: &str,
        error_rate: f64,
        p99_ms: f64,
    ) -> PostmortemBundle {
        let timeline = vec![
            "T-0: Progressive Canary Ring 0 traffic initiated.".to_string(),
            format!(
                "T+2m: Anomaly detected: Error rate rose to {:.2}%, P99 reached {:.1}ms.",
                error_rate, p99_ms
            ),
            "T+3m: Automated circuit breaker tripped. Autonomous rollback executed.".to_string(),
            "T+4m: Production traffic restored to baseline healthy state.".to_string(),
        ];

        let action_items = vec![
            "AI-1: Add chaos fault injection test covering downstream database failover."
                .to_string(),
            "AI-2: Refine client retry loop with Full Jitter backoff.".to_string(),
        ];

        let markdown = format!(
            "# Blameless Postmortem: {}\n\n## Root Cause\n{}\n\n## Timeline\n{}\n\n## Action Items\n{}",
            service,
            root_cause,
            timeline
                .iter()
                .map(|t| format!("- {}", t))
                .collect::<Vec<_>>()
                .join("\n"),
            action_items
                .iter()
                .map(|a| format!("- [ ] {}", a))
                .collect::<Vec<_>>()
                .join("\n")
        );

        PostmortemBundle {
            service_name: service.to_string(),
            root_cause: root_cause.to_string(),
            incident_timeline: timeline,
            action_items,
            markdown_report: markdown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generates_detailed_postmortem() {
        let generator = PostmortemGenerator::new();
        let postmortem = generator.generate_postmortem(
            "gateway-service",
            "Memory leak in regex engine",
            8.5,
            620.0,
        );
        assert!(postmortem.markdown_report.contains("Blameless Postmortem"));
        assert_eq!(postmortem.action_items.len(), 2);
    }
}
