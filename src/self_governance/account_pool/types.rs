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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub type AffinityCacheMap = HashMap<String, (String, Instant)>; // affinity_key -> (account_id, expires_at)
