use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DeploymentEnvironment {
    Dev,
    Staging,
    Canary,
    Production,
}

impl fmt::Display for DeploymentEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeploymentEnvironment::Dev => write!(f, "dev"),
            DeploymentEnvironment::Staging => write!(f, "staging"),
            DeploymentEnvironment::Canary => write!(f, "canary"),
            DeploymentEnvironment::Production => write!(f, "production"),
        }
    }
}

impl DeploymentEnvironment {
    pub fn canonical_branch_name(&self) -> &'static str {
        match self {
            DeploymentEnvironment::Dev => "dev",
            DeploymentEnvironment::Staging => "staging",
            DeploymentEnvironment::Canary => "canary",
            DeploymentEnvironment::Production => "production",
        }
    }

    pub fn from_branch_name(branch: &str) -> Option<Self> {
        match branch.trim_start_matches("refs/heads/") {
            "dev" | "environment/dev" => Some(DeploymentEnvironment::Dev),
            "staging" | "environment/staging" => Some(DeploymentEnvironment::Staging),
            "canary" | "environment/canary" => Some(DeploymentEnvironment::Canary),
            "production" | "prod" | "environment/production" => {
                Some(DeploymentEnvironment::Production)
            }
            _ => None,
        }
    }

    /// Next required promotional tier in the hyperscaler continuous delivery lifecycle
    pub fn next_tier(&self) -> Option<Self> {
        match self {
            DeploymentEnvironment::Dev => Some(DeploymentEnvironment::Staging),
            DeploymentEnvironment::Staging => Some(DeploymentEnvironment::Canary),
            DeploymentEnvironment::Canary => Some(DeploymentEnvironment::Production),
            DeploymentEnvironment::Production => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentPromotionGateResult {
    pub is_eligible: bool,
    pub source_env: DeploymentEnvironment,
    pub target_env: DeploymentEnvironment,
    pub bake_window_minutes_observed: u64,
    pub minimum_bake_window_minutes: u64,
    pub observed_error_burn_rate: f64,
    pub max_allowed_burn_rate: f64,
    pub violations: Vec<String>,
}

pub struct EnvironmentPromotionPolicy;

impl EnvironmentPromotionPolicy {
    /// Validates whether an environment promotion from source to target satisfies all hyperscaler gates
    pub fn validate_promotion(
        source_env: DeploymentEnvironment,
        target_env: DeploymentEnvironment,
        bake_window_minutes: u64,
        observed_burn_rate: f64,
        integration_tests_passed: bool,
        all_images_pinned: bool,
    ) -> Result<EnvironmentPromotionGateResult> {
        let mut violations = Vec::new();

        // Rule 1: Enforce strict promotion DAG progression (dev -> staging -> canary -> production)
        if source_env.next_tier() != Some(target_env) {
            violations.push(format!(
                "Invalid promotion progression: cannot promote directly from '{}' to '{}'. Must follow strict DAG: dev -> staging -> canary -> production.",
                source_env, target_env
            ));
        }

        // Rule 2: Minimum bake windows by target environment
        let min_bake_minutes = match target_env {
            DeploymentEnvironment::Dev => 0,
            DeploymentEnvironment::Staging => 15,
            DeploymentEnvironment::Canary => 30,
            DeploymentEnvironment::Production => 60, // Minimum 1-hour canary bake before broad production rollout
        };

        if bake_window_minutes < min_bake_minutes {
            violations.push(format!(
                "Bake window violation for '{}': observed {}m, but minimum required bake duration is {}m.",
                target_env, bake_window_minutes, min_bake_minutes
            ));
        }

        // Rule 3: Google SRE Multi-Burn-Rate Gate
        let max_burn_rate = match target_env {
            DeploymentEnvironment::Production => 1.0, // Error budget burn rate must not exceed 1.0x in production
            DeploymentEnvironment::Canary => 2.0,
            _ => 10.0,
        };

        if observed_burn_rate > max_burn_rate {
            violations.push(format!(
                "SRE Error Budget Burn Rate Exceeded for '{}': observed {:.2}x (max permitted: {:.2}x). Automated release freeze active.",
                target_env, observed_burn_rate, max_burn_rate
            ));
        }

        // Rule 4: Integration tests & image digest pinning
        if !integration_tests_passed {
            violations.push(format!(
                "Integration test failure: all unit, regression, and e2e test suites on '{}' must be 100% green.",
                source_env
            ));
        }

        if !all_images_pinned {
            violations.push(format!(
                "Supply chain violation: promotion to '{}' requires 100% of container images to be pinned to immutable sha256 digests.",
                target_env
            ));
        }

        let is_eligible = violations.is_empty();

        if !is_eligible {
            bail!(
                "Environment promotion blocked from '{}' to '{}': {}",
                source_env,
                target_env,
                violations.join("; ")
            );
        }

        Ok(EnvironmentPromotionGateResult {
            is_eligible: true,
            source_env,
            target_env,
            bake_window_minutes_observed: bake_window_minutes,
            minimum_bake_window_minutes: min_bake_minutes,
            observed_error_burn_rate: observed_burn_rate,
            max_allowed_burn_rate: max_burn_rate,
            violations: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_promotion_dag_canary_to_production() {
        let res = EnvironmentPromotionPolicy::validate_promotion(
            DeploymentEnvironment::Canary,
            DeploymentEnvironment::Production,
            90,  // 90 minutes observed bake (meets 60m min)
            0.4, // 0.4x burn rate (meets 1.0x max)
            true,
            true,
        );

        assert!(res.is_ok());
        let gate = res.unwrap();
        assert!(gate.is_eligible);
        assert_eq!(gate.minimum_bake_window_minutes, 60);
    }

    #[test]
    fn test_blocks_invalid_skip_staging_to_production() {
        let res = EnvironmentPromotionPolicy::validate_promotion(
            DeploymentEnvironment::Staging,
            DeploymentEnvironment::Production, // Skipping Canary!
            120,
            0.1,
            true,
            true,
        );

        assert!(res.is_err());
        let err = res.unwrap_err().to_string();
        assert!(err.contains("Invalid promotion progression"));
    }

    #[test]
    fn test_blocks_canary_to_production_when_burn_rate_exceeded() {
        let res = EnvironmentPromotionPolicy::validate_promotion(
            DeploymentEnvironment::Canary,
            DeploymentEnvironment::Production,
            120,
            6.5, // 6.5x burn rate (breaches 1.0x max)
            true,
            true,
        );

        assert!(res.is_err());
        let err = res.unwrap_err().to_string();
        assert!(err.contains("SRE Error Budget Burn Rate Exceeded"));
    }
}
