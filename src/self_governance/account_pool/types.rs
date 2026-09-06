use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use crate::ai_driver::provider::ModelProvider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub timestamp: DateTime<Utc>,
    pub model: String,
    pub tokens_consumed: usize,
    pub estimated_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AuthType {
    #[default]
    CliPassthrough,
    OAuthToken,
    ConfigDirectory,
    ApiKey,
}

impl AuthType {
    pub fn from_str_opt(val: Option<&str>) -> Self {
        match val.map(|s| s.to_lowercase()).as_deref() {
            Some("oauth") | Some("oauth_token") | Some("token") => AuthType::OAuthToken,
            Some("config_dir") | Some("dir") | Some("profile_dir") => AuthType::ConfigDirectory,
            Some("api_key") | Some("key") => AuthType::ApiKey,
            _ => AuthType::CliPassthrough,
        }
    }
}

#[derive(Clone)]
pub struct ManagedAccount {
    pub account_id: String,
    pub provider: ModelProvider,
    pub auth_type: AuthType,
    pub auth_profile_or_key: Option<String>,
    pub oauth_token: Option<String>,
    pub config_dir: Option<String>,
    pub max_5hr_tokens: Option<usize>,
    pub max_weekly_budget_usd: Option<f64>,
    pub usage_history: VecDeque<UsageRecord>,
    pub cooldown_until: Option<Instant>,
    pub last_leased_at: Instant,
    pub is_draining: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountQuotaView {
    pub account_id: String,
    pub provider: String,
    pub auth_type: Option<String>,
    pub used_5hr_tokens: usize,
    pub max_5hr_tokens: Option<usize>,
    pub remaining_5hr_tokens: Option<usize>,
    pub pct_5hr_used: Option<f64>,
    pub weekly_spent_usd: f64,
    pub weekly_budget_usd: Option<f64>,
    pub pct_weekly_spent: Option<f64>,
    pub quota_description: String,
    pub is_active: bool,
    pub is_draining: bool,
    pub lifecycle_state: String,
    pub cooldown_remaining_secs: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AddAccountPayload {
    pub account_id: String,
    pub provider: String,
    pub auth_type: Option<String>,
    pub auth_profile_or_key: Option<String>,
    pub oauth_token: Option<String>,
    pub config_dir: Option<String>,
    pub max_5hr_tokens: Option<usize>,
    pub max_weekly_budget_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainAccountPayload {
    pub account_id: String,
}

pub type AccountPoolMap = HashMap<ModelProvider, Vec<Arc<RwLock<ManagedAccount>>>>;

/// Hand-written so a credential can never reach a log through `{:?}`.
///
/// `ManagedAccount` previously derived `Debug` while holding `oauth_token` and
/// `auth_profile_or_key` as plain `String`s. Nothing logs the whole struct
/// today, but the derive made it one careless `{:?}` away — and a token in a
/// log line is a token on disk (invariant I6).
impl std::fmt::Debug for ManagedAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedAccount")
            .field("account_id", &self.account_id)
            .field("provider", &self.provider)
            .field("auth_type", &self.auth_type)
            .field("auth_profile_or_key", &redacted(&self.auth_profile_or_key))
            .field("oauth_token", &redacted(&self.oauth_token))
            .field("config_dir", &self.config_dir)
            .field("max_5hr_tokens", &self.max_5hr_tokens)
            .field("max_weekly_budget_usd", &self.max_weekly_budget_usd)
            .field("usage_records", &self.usage_history.len())
            .field("cooldown_until", &self.cooldown_until)
            .field("is_draining", &self.is_draining)
            .finish()
    }
}

impl std::fmt::Debug for AddAccountPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AddAccountPayload")
            .field("account_id", &self.account_id)
            .field("provider", &self.provider)
            .field("auth_type", &self.auth_type)
            .field("auth_profile_or_key", &redacted(&self.auth_profile_or_key))
            .field("oauth_token", &redacted(&self.oauth_token))
            .field("config_dir", &self.config_dir)
            .field("max_5hr_tokens", &self.max_5hr_tokens)
            .field("max_weekly_budget_usd", &self.max_weekly_budget_usd)
            .finish()
    }
}

/// Reports only whether a secret is present, never any part of its value.
///
/// Deliberately not a prefix or a length: both leak information about the
/// credential, and a prefix identifies the issuing provider and token family.
fn redacted(v: &Option<String>) -> &'static str {
    match v {
        Some(_) => "[REDACTED]",
        None => "None",
    }
}

#[cfg(test)]
mod redaction_tests {
    use super::*;
    use std::collections::VecDeque;
    use std::time::Instant;

    fn account_with_secrets() -> ManagedAccount {
        ManagedAccount {
            account_id: "claude:seat-1".to_string(),
            provider: ModelProvider::AnthropicClaudeCode,
            auth_type: AuthType::OAuthToken,
            auth_profile_or_key: Some("sk-ant-api03-SUPERSECRETKEYVALUE".to_string()),
            oauth_token: Some("sk-ant-oat01-SUPERSECRETTOKENVALUE".to_string()),
            config_dir: None,
            max_5hr_tokens: None,
            max_weekly_budget_usd: None,
            usage_history: VecDeque::new(),
            cooldown_until: None,
            last_leased_at: Instant::now(),
            is_draining: false,
        }
    }

    #[test]
    fn debug_never_emits_credential_material() {
        let rendered = format!("{:?}", account_with_secrets());
        assert!(!rendered.contains("SUPERSECRETTOKENVALUE"));
        assert!(!rendered.contains("SUPERSECRETKEYVALUE"));
        assert!(!rendered.contains("sk-ant-"));
        assert!(rendered.contains("[REDACTED]"));
        // Non-secret fields must still be useful for debugging.
        assert!(rendered.contains("claude:seat-1"));
    }

    #[test]
    fn debug_distinguishes_absent_from_redacted() {
        let mut acc = account_with_secrets();
        acc.oauth_token = None;
        let rendered = format!("{:?}", acc);
        assert!(rendered.contains("oauth_token: \"None\""));
    }

    #[test]
    fn payload_debug_also_redacts() {
        let p = AddAccountPayload {
            account_id: "x".to_string(),
            provider: "claude".to_string(),
            auth_type: Some("oauth".to_string()),
            auth_profile_or_key: None,
            oauth_token: Some("sk-ant-oat01-LEAKME".to_string()),
            config_dir: None,
            max_5hr_tokens: None,
            max_weekly_budget_usd: None,
        };
        let rendered = format!("{:?}", p);
        assert!(!rendered.contains("LEAKME"));
        assert!(rendered.contains("[REDACTED]"));
    }
}
