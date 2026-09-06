use chrono::{Duration as ChronoDuration, Utc};
use std::time::Instant;

use super::types::{AccountQuotaView, ManagedAccount};

/// Computes a snapshot AccountQuotaView from a ManagedAccount's current state.
/// Centralizes the sliding-window quota math shared by record_spend and get_pool_status_views.
pub fn compute_quota_view(acc: &ManagedAccount) -> AccountQuotaView {
    let utc_now = Utc::now();
    let five_hours_ago = utc_now - ChronoDuration::hours(5);
    let seven_days_ago = utc_now - ChronoDuration::days(7);
    let now = Instant::now();

    let used_5hr: usize = acc
        .usage_history
        .iter()
        .filter(|r| r.timestamp >= five_hours_ago)
        .map(|r| r.tokens_consumed)
        .sum();

    let spent_weekly: f64 = acc
        .usage_history
        .iter()
        .filter(|r| r.timestamp >= seven_days_ago)
        .map(|r| r.estimated_cost_usd)
        .sum();

    let spent_clean = spent_weekly.abs().max(0.0);

    let (pct_5hr, rem_5hr) = match acc.max_5hr_tokens {
        Some(max) => (
            Some((used_5hr as f64 / max.max(1) as f64) * 100.0),
            Some(max.saturating_sub(used_5hr)),
        ),
        None => (None, None),
    };

    let pct_weekly = acc
        .max_weekly_budget_usd
        .map(|b| (spent_clean / b.max(0.01)) * 100.0);

    let quota_desc = match (acc.max_5hr_tokens, acc.max_weekly_budget_usd) {
        (Some(max), Some(b)) => {
            format!("{:.0}k 5hr / ${:.0} wk", max as f64 / 1000.0, b)
        }
        (Some(max), None) => format!("{:.0}k 5hr / Uncapped", max as f64 / 1000.0),
        (None, Some(b)) => format!("Uncapped 5hr / ${:.0} wk", b),
        (None, None) => "Dynamic CLI Subscription (Uncapped)".to_string(),
    };

    let cooldown_secs = acc
        .cooldown_until
        .map(|c| c.saturating_duration_since(now).as_secs())
        .unwrap_or(0);

    let is_exhausted = acc
        .max_5hr_tokens
        .map(|max| used_5hr >= max)
        .unwrap_or(false);

    let state_str = if acc.is_draining {
        "DRAINING".to_string()
    } else if cooldown_secs > 0 {
        format!("COOLDOWN ({}s)", cooldown_secs)
    } else if is_exhausted {
        "QUOTA_EXHAUSTED".to_string()
    } else {
        "ACTIVE".to_string()
    };

    AccountQuotaView {
        account_id: acc.account_id.clone(),
        provider: acc.provider.display_name().to_string(),
        auth_type: Some(format!("{:?}", acc.auth_type)),
        used_5hr_tokens: used_5hr,
        max_5hr_tokens: acc.max_5hr_tokens,
        remaining_5hr_tokens: rem_5hr,
        pct_5hr_used: pct_5hr,
        weekly_spent_usd: spent_clean,
        weekly_budget_usd: acc.max_weekly_budget_usd,
        pct_weekly_spent: pct_weekly,
        quota_description: quota_desc,
        is_active: !acc.is_draining && acc.cooldown_until.is_none() && !is_exhausted,
        is_draining: acc.is_draining,
        lifecycle_state: state_str,
        cooldown_remaining_secs: cooldown_secs,
    }
}
