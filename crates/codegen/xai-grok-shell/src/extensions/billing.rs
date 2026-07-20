//! `chutes.build/billing` extension handler.
//!
//! Adapts Chutes account usage to the pager's compact usage surface.

use agent_client_protocol as acp;
use serde::{Deserialize, Serialize};

use super::{ExtResult, to_raw_response};
use crate::agent::MvpAgent;

/// Billing period cycle identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingCycle {
    pub year: i32,
    pub month: i32,
}

/// Cent value from the billing API (USD cents).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cent {
    /// proto3 JSON omits zero-valued scalars, so a `$0` Cent arrives as `{}`;
    /// default to 0 rather than failing the whole parse.
    #[serde(default)]
    pub val: i64,
}

/// A usage period (weekly or monthly) from the newer credits config.
///
/// `start`/`end` are RFC 3339 timestamps. `period_type` is the proto enum name
/// (e.g. `USAGE_PERIOD_TYPE_WEEKLY`); kept so callers can distinguish weekly
/// vs monthly cycles.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsagePeriod {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub period_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
}

/// One independently enforced Chutes usage window.
///
/// Chutes subscriptions can expose several simultaneous limits (notably the
/// rolling four-hour window and the monthly billing-cycle cap). Keeping every
/// window on the wire lets detailed clients render the complete account state,
/// while `credit_usage_percent`/`current_period` remain the most constrained
/// window for compact indicators and warnings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub period_type: String,
    pub usage_percent: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_at: Option<String>,
}

/// Usage summary for one past billing period.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingPeriodUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_cycle: Option<BillingCycle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub included_used: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_demand_used: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_used: Option<Cent>,
}

/// Current billing configuration for Grok Build coding credits.
///
/// Carries both the newer credits-config fields (`credit_usage_percent`,
/// `current_period`) and the deprecated `GrokBuildBillingConfig` fields
/// (`monthly_limit`, `used`, `billing_period_*`). Consumers should prefer the
/// new fields and fall back to the deprecated ones, so the same struct works
/// against both the new `GetGrokCreditsConfig` and the legacy
/// `GetGrokBuildBillingConfig` backend responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingConfig {
    /// Included credit usage as a percentage of the allowance (0.0–100.0).
    /// Preferred over deriving from `monthly_limit`/`used`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_usage_percent: Option<f64>,
    /// Current usage period (weekly or monthly). Preferred over
    /// `billing_period_start`/`billing_period_end`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_period: Option<UsagePeriod>,
    /// All independently enforced active windows. The compact UI continues to
    /// use `current_period`; `/usage` renders this collection in full.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub usage_windows: Vec<UsageWindow>,
    /// Deprecated: included monthly credit budget. Use `credit_usage_percent`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly_limit: Option<Cent>,
    /// Deprecated: credits used this period. Use `credit_usage_percent`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_demand_cap: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_demand_used: Option<Cent>,
    /// Remaining prepaid (purchased) credit balance, positive — the "bought
    /// credits" the user has topped up. Populated from the credits config
    /// (`GetGrokCreditsConfig.prepaid_balance`); absent in the legacy billing
    /// shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepaid_balance: Option<Cent>,
    /// Whether this user is on unified usage billing (shared weekly/monthly
    /// pool). From `GrokCreditsConfig.is_unified_billing_user`, which billing
    /// sets from remote settings `unified_consumer_billing_enabled`. `None` when
    /// absent (legacy `GetGrokBuildBillingConfig` shape or older servers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_unified_billing_user: Option<bool>,
    /// Deprecated: use `current_period.start`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_period_start: Option<String>,
    /// Deprecated: use `current_period.end`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_period_end: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<BillingPeriodUsage>,
}

/// Top-level response (primarily from `GET /rest/grok/credits` + auto-topup-rule).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingConfigResponse {
    pub config: Option<BillingConfig>,
    /// Whether on-demand credit usage is enabled. When `false`, the pager
    /// should hide on-demand controls. Populated from `RemoteSettings`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_demand_enabled: Option<bool>,
    /// User-friendly subscription tier name (e.g. "SuperGrok Heavy").
    /// Populated from `RemoteSettings` so the pager can update its cached
    /// tier on every billing fetch without an extra request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_tier: Option<String>,
}

/// Auto top-up configuration (from GetAutoTopupRule).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoTopupRule {
    /// proto3 JSON omits `false`, so a disabled rule arrives without this field;
    /// default to `false` rather than failing the parse (which would otherwise
    /// keep a stale cached rule in the pager).
    #[serde(default)]
    pub enabled: bool,
    pub min_before_hitting_sl: Option<Cent>,
    pub topup_amount: Option<Cent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_amount_per_month: Option<Cent>,
}

/// Wrapper for the auto top-up rule response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAutoTopupRuleResponse {
    #[serde(default)]
    pub rule: Option<AutoTopupRule>,
}

#[tracing::instrument(skip_all, fields(method = %args.method))]
pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    match args.method.as_ref() {
        "chutes.build/billing" => {
            tracing::info!("handling billing config request");
            handle_get_billing(agent).await
        }
        "chutes.build/auto-topup-rule" => {
            tracing::info!("handling auto top-up rule request");
            handle_get_auto_topup_rule(agent).await
        }
        _ => Err(acp::Error::method_not_found()),
    }
}

/// Structured context for unified-log entries from a successful billing fetch.
///
/// Keeps history to a count + the most recent period so `~/.chutes-build/logs/unified.jsonl`
/// stays useful without dumping unbounded period arrays.
#[cfg(test)]
fn billing_unified_log_ctx(billing: &BillingConfigResponse) -> serde_json::Value {
    let history_len = billing
        .config
        .as_ref()
        .map(|c| c.history.len())
        .unwrap_or(0);
    let latest_history = billing
        .config
        .as_ref()
        .and_then(|c| c.history.last())
        .and_then(|p| serde_json::to_value(p).ok());

    let mut config_value = billing
        .config
        .as_ref()
        .and_then(|c| serde_json::to_value(c).ok())
        .unwrap_or(serde_json::Value::Null);
    if let Some(obj) = config_value.as_object_mut() {
        // Drop full history array; surface length + latest entry instead.
        obj.remove("history");
        obj.insert("historyLen".into(), serde_json::json!(history_len));
        if let Some(latest) = latest_history {
            obj.insert("latestHistory".into(), latest);
        }
    }

    serde_json::json!({
        "config": config_value,
        "onDemandEnabled": billing.on_demand_enabled,
        "subscriptionTier": billing.subscription_tier,
    })
}

async fn handle_get_billing(_agent: &MvpAgent) -> ExtResult {
    let snapshot = chutes_build_core::account::ChutesAccountClient::from_env()
        .map_err(|error| {
            tracing::warn!(%error, "Chutes usage client is unavailable");
            acp::Error::invalid_request()
                .data("Chutes usage requires CHUTES_API_KEY or `chutes-build login`.")
        })?
        .usage_snapshot(false)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Chutes usage request failed");
            acp::Error::internal_error().data("Unable to fetch Chutes usage data.")
        })?;
    to_raw_response(&billing_from_chutes_snapshot(&snapshot))
}

async fn handle_get_auto_topup_rule(_agent: &MvpAgent) -> ExtResult {
    to_raw_response(&GetAutoTopupRuleResponse { rule: None })
}

fn billing_from_chutes_snapshot(snapshot: &serde_json::Value) -> BillingConfigResponse {
    let raw_subscription = &snapshot["subscription_usage"];
    let subscription = find_best_usage_object(raw_subscription, 0)
        .map(|(value, _)| value)
        .unwrap_or(raw_subscription);
    let candidates = [
        usage_sample(
            subscription,
            &[
                "billing_cycle_cap",
                "monthly_cap",
                "monthly_window",
                "billing_cycle",
                "monthly",
            ],
            &["billing_cycle_used", "monthly_used"],
            &["billing_cycle_limit", "monthly_limit", "monthly_cap_usd"],
            "CHUTES_USAGE_PERIOD_MONTHLY",
        ),
        usage_sample(
            subscription,
            &[
                "four_hour_window",
                "rolling_4h_window",
                "four_hour_cap",
                "rolling_window",
                "four_hour",
            ],
            &["four_hour_used", "rolling_4h_used"],
            &["four_hour_limit", "rolling_4h_limit", "four_hour_cap_usd"],
            "CHUTES_USAGE_PERIOD_FOUR_HOUR",
        ),
        usage_sample(
            subscription,
            &["weekly_window", "weekly_cap"],
            &["weekly_used"],
            &["weekly_limit"],
            "CHUTES_USAGE_PERIOD_WEEKLY",
        ),
        usage_sample(
            subscription,
            &[
                "daily_quota_usage",
                "daily_quota",
                "daily_window",
                "daily_requests",
            ],
            &["daily_used", "daily_quota_used"],
            &["daily_limit", "daily_request_limit", "daily_quota_limit"],
            "CHUTES_USAGE_PERIOD_DAILY",
        ),
        quota_usage_sample(&snapshot["quota_usage"], &snapshot["quotas"]),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    // A direct daily window and the documented quota endpoints can both
    // describe the same enforcement period. Keep one entry per type, choosing
    // the more constrained sample so the UI never duplicates a window.
    let mut samples: Vec<UsageSample> = Vec::new();
    for candidate in candidates {
        if let Some(existing) = samples
            .iter_mut()
            .find(|sample| sample.period_type == candidate.period_type)
        {
            if candidate.percent > existing.percent {
                *existing = candidate;
            }
        } else {
            samples.push(candidate);
        }
    }
    let active = samples
        .iter()
        .max_by(|left, right| left.percent.total_cmp(&right.percent));
    let credit_usage_percent = active.map(|sample| sample.percent);
    let current_period = active.map(|sample| UsagePeriod {
        period_type: Some(sample.period_type.to_owned()),
        start: None,
        end: sample.reset.clone(),
    });
    let usage_windows = samples
        .iter()
        .map(|sample| UsageWindow {
            period_type: sample.period_type.to_owned(),
            usage_percent: sample.percent,
            reset_at: sample.reset.clone(),
        })
        .collect();
    let subscription_tier = plan_name(subscription);

    BillingConfigResponse {
        config: Some(BillingConfig {
            credit_usage_percent,
            current_period,
            usage_windows,
            monthly_limit: None,
            used: None,
            on_demand_cap: None,
            on_demand_used: None,
            prepaid_balance: None,
            is_unified_billing_user: None,
            billing_period_start: None,
            billing_period_end: None,
            history: Vec::new(),
        }),
        on_demand_enabled: None,
        subscription_tier,
    }
}

#[derive(Debug)]
struct UsageSample {
    percent: f64,
    reset: Option<String>,
    period_type: &'static str,
}

fn usage_sample(
    payload: &serde_json::Value,
    object_keys: &[&str],
    direct_used_keys: &[&str],
    direct_limit_keys: &[&str],
    period_type: &'static str,
) -> Option<UsageSample> {
    let nested = first_object(payload, object_keys);
    let used = nested
        .and_then(|value| first_number(value, &["usage", "used", "consumed"]))
        .or_else(|| first_number(payload, direct_used_keys));
    let limit = nested
        .and_then(|value| first_number(value, &["cap", "limit", "quota", "total"]))
        .or_else(|| first_number(payload, direct_limit_keys));
    let percent = nested
        .and_then(|value| first_number(value, &["usage_percent", "percent", "percentage"]))
        .or_else(|| {
            let (used, limit) = (used?, limit?);
            (limit > 0.0).then_some(used / limit * 100.0)
        })?;
    let reset = nested
        .and_then(|value| first_string(value, &["reset_at", "reset_label", "end", "expires_at"]));
    Some(UsageSample {
        percent,
        reset,
        period_type,
    })
}

fn quota_usage_sample(
    quota_usage: &serde_json::Value,
    quotas: &serde_json::Value,
) -> Option<UsageSample> {
    let (used, reported_limit) = aggregate_quota_usage(quota_usage);
    let used = used?;
    let limit = reported_limit.or_else(|| aggregate_quota_limit(quotas))?;
    if limit <= 0.0 {
        return None;
    }
    Some(UsageSample {
        percent: used / limit * 100.0,
        reset: None,
        period_type: "CHUTES_USAGE_PERIOD_DAILY",
    })
}

fn aggregate_quota_usage(value: &serde_json::Value) -> (Option<f64>, Option<f64>) {
    if let Some(object) = value.as_object() {
        let direct_used = first_number(value, &["used"]);
        let direct_quota = first_number(value, &["quota", "limit"]);
        if direct_used.is_some() || direct_quota.is_some() {
            return (direct_used, direct_quota);
        }
        let mut used: Option<f64> = None;
        let mut quota: Option<f64> = None;
        for entry in object.values().filter(|entry| entry.is_object()) {
            if let Some(next) = first_number(entry, &["used"]) {
                used = Some(used.unwrap_or(0.0) + next);
            }
            if let Some(next) = first_number(entry, &["quota", "limit"]) {
                quota = Some(quota.unwrap_or(0.0) + next);
            }
        }
        return (used, quota);
    }
    (None, None)
}

fn aggregate_quota_limit(value: &serde_json::Value) -> Option<f64> {
    let items = value.as_array().or_else(|| {
        value
            .get("items")
            .and_then(serde_json::Value::as_array)
            .or_else(|| value.get("quotas").and_then(serde_json::Value::as_array))
    })?;
    let mut total: Option<f64> = None;
    for entry in items {
        if let Some(limit) = first_number(entry, &["quota", "limit"]) {
            total = Some(total.unwrap_or(0.0) + limit);
        }
    }
    total
}

fn plan_name(payload: &serde_json::Value) -> Option<String> {
    let plan = payload.get("plan").filter(|value| value.is_object());
    let source = plan.unwrap_or(payload);
    first_string(source, &["name", "plan_name", "tier"])
        .or_else(|| first_string(payload, &["plan_name", "subscription_tier", "tier", "name"]))
        .or_else(|| {
            if payload
                .get("subscription")
                .and_then(|value| value.as_bool())
                == Some(false)
            {
                return Some("Free tier".to_owned());
            }
            match first_number(source, &["monthly_price"])
                .or_else(|| first_number(payload, &["monthly_price"]))
            {
                Some(price) if (price - 10.0).abs() < f64::EPSILON => Some("Plus".to_owned()),
                Some(price) if (price - 20.0).abs() < f64::EPSILON => Some("Pro".to_owned()),
                _ if payload.get("custom").and_then(|value| value.as_bool()) == Some(true) => {
                    Some("Custom".to_owned())
                }
                _ if payload
                    .get("subscription")
                    .and_then(|value| value.as_bool())
                    == Some(true)
                    && payload.get("custom").and_then(|value| value.as_bool()) == Some(false) =>
                {
                    Some("Paid tier".to_owned())
                }
                _ => None,
            }
        })
}

fn find_best_usage_object(
    value: &serde_json::Value,
    depth: usize,
) -> Option<(&serde_json::Value, usize)> {
    if depth > 6 {
        return None;
    }
    let object = value.as_object()?;
    let signals = [
        "billing_cycle_cap",
        "monthly",
        "four_hour_window",
        "rolling_4h_window",
        "daily_quota_usage",
        "daily_quota",
        "weekly_window",
        "plan",
        "plan_name",
    ];
    let score = signals
        .iter()
        .filter(|key| object.contains_key(**key))
        .count();
    let mut best = (score > 0).then_some((value, score));
    for child in object.values().filter(|child| child.is_object()) {
        if let Some(candidate) = find_best_usage_object(child, depth + 1)
            && best.map_or(true, |(_, best_score)| candidate.1 > best_score)
        {
            best = Some(candidate);
        }
    }
    best
}

fn first_object<'a>(value: &'a serde_json::Value, keys: &[&str]) -> Option<&'a serde_json::Value> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .filter(|value| value.is_object())
}

fn first_number(value: &serde_json::Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        let value = value.get(*key)?;
        value
            .as_f64()
            .or_else(|| value.as_str()?.parse::<f64>().ok())
    })
}

fn first_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key)?.as_str().map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chutes_snapshot_maps_usage_without_profile_data() {
        let snapshot = serde_json::json!({
            "subscription_usage": {
                "plan_name": "Developer",
                "billing_cycle_cap": {
                    "used": 25,
                    "limit": 100,
                    "reset_at": "2026-08-01T00:00:00Z"
                }
            },
            "quotas": [],
            "quota_usage": null,
            "model_stats": null
        });
        let response = billing_from_chutes_snapshot(&snapshot);
        let config = response.config.expect("usage config");
        assert_eq!(config.credit_usage_percent, Some(25.0));
        assert_eq!(response.subscription_tier.as_deref(), Some("Developer"));
        assert_eq!(
            config
                .current_period
                .and_then(|period| period.end)
                .as_deref(),
            Some("2026-08-01T00:00:00Z")
        );
    }

    #[test]
    fn chutes_snapshot_accepts_numeric_strings_and_daily_fallback() {
        let snapshot = serde_json::json!({
            "subscription_usage": {
                "daily_quota_usage": {"used": "50", "limit": "200"}
            }
        });
        let response = billing_from_chutes_snapshot(&snapshot);
        assert_eq!(
            response
                .config
                .and_then(|config| config.credit_usage_percent),
            Some(25.0)
        );
    }

    #[test]
    fn chutes_snapshot_selects_the_most_constrained_window() {
        let snapshot = serde_json::json!({
            "subscription_usage": {
                "subscription": true,
                "custom": false,
                "monthly_price": 20,
                "monthly": {
                    "usage": 25,
                    "cap": 100,
                    "reset_at": "2026-08-01T00:00:00Z"
                },
                "four_hour": {
                    "usage": 9,
                    "cap": 10,
                    "reset_at": "2026-07-19T16:00:00Z"
                }
            },
            "quotas": [],
            "quota_usage": null
        });
        let response = billing_from_chutes_snapshot(&snapshot);
        let config = response.config.expect("usage config");
        assert_eq!(response.subscription_tier.as_deref(), Some("Pro"));
        assert_eq!(config.credit_usage_percent, Some(90.0));
        assert_eq!(
            config
                .usage_windows
                .iter()
                .map(|window| (window.period_type.as_str(), window.usage_percent))
                .collect::<Vec<_>>(),
            vec![
                ("CHUTES_USAGE_PERIOD_MONTHLY", 25.0),
                ("CHUTES_USAGE_PERIOD_FOUR_HOUR", 90.0),
            ]
        );
        assert_eq!(
            config.usage_windows[1].reset_at.as_deref(),
            Some("2026-07-19T16:00:00Z")
        );
        assert_eq!(
            config.current_period.and_then(|period| period.period_type),
            Some("CHUTES_USAGE_PERIOD_FOUR_HOUR".to_owned())
        );
    }

    #[test]
    fn chutes_snapshot_aggregates_documented_per_chute_quota_usage() {
        let snapshot = serde_json::json!({
            "subscription_usage": {"subscription": false},
            "quotas": {
                "items": [
                    {"chute_id": "*", "quota": 100},
                    {"chute_id": "image", "quota": 50}
                ]
            },
            "quota_usage": {
                "*": {"used": 80, "quota": 100},
                "image": {"used": 10, "quota": 50}
            }
        });
        let response = billing_from_chutes_snapshot(&snapshot);
        let config = response.config.expect("usage config");
        assert_eq!(response.subscription_tier.as_deref(), Some("Free tier"));
        assert_eq!(config.credit_usage_percent, Some(60.0));
        assert_eq!(
            config.current_period.and_then(|period| period.period_type),
            Some("CHUTES_USAGE_PERIOD_DAILY".to_owned())
        );
    }

    #[test]
    fn chutes_snapshot_unwraps_nested_usage_payloads() {
        let snapshot = serde_json::json!({
            "subscription_usage": {
                "data": {
                    "plan": {"name": "Enterprise"},
                    "daily_quota_usage": {"used": 3, "limit": 10}
                }
            },
            "quotas": [],
            "quota_usage": null
        });
        let response = billing_from_chutes_snapshot(&snapshot);
        assert_eq!(response.subscription_tier.as_deref(), Some("Enterprise"));
        assert_eq!(
            response
                .config
                .and_then(|config| config.credit_usage_percent),
            Some(30.0)
        );
    }

    #[test]
    fn auto_topup_disabled_rule_omits_enabled_field() {
        // proto3 JSON omits `false` / `0`, so a disabled rule arrives without
        // `enabled` (and zero Cents as `{}`). It must still deserialize (as
        // disabled) rather than erroring — otherwise the pager keeps a stale
        // cached rule.
        let json = serde_json::json!({
            "rule": { "topupAmount": {"val": 500}, "minBeforeHittingSl": {} }
        });
        let resp: GetAutoTopupRuleResponse = serde_json::from_value(json).unwrap();
        let rule = resp.rule.expect("rule present");
        assert!(!rule.enabled);
        assert_eq!(rule.topup_amount.unwrap().val, 500);
        assert_eq!(rule.min_before_hitting_sl.unwrap().val, 0);
    }

    #[test]
    fn billing_config_response_deserializes_from_backend_json() {
        let json = serde_json::json!({
            "config": {
                "monthlyLimit": {"val": 2000},
                "used": {"val": 1234},
                "onDemandCap": {"val": 500},
                "billingPeriodStart": "2025-04-01T00:00:00Z",
                "billingPeriodEnd": "2025-05-01T00:00:00Z",
                "history": [
                    {
                        "billingCycle": {"year": 2025, "month": 3},
                        "includedUsed": {"val": 1800},
                        "onDemandUsed": {"val": 0},
                        "totalUsed": {"val": 1800}
                    }
                ]
            }
        });
        let resp: BillingConfigResponse = serde_json::from_value(json).unwrap();
        let config = resp.config.unwrap();
        assert_eq!(config.monthly_limit.unwrap().val, 2000);
        assert_eq!(config.used.unwrap().val, 1234);
        assert_eq!(config.on_demand_cap.unwrap().val, 500);
        assert_eq!(
            config.billing_period_start.as_deref(),
            Some("2025-04-01T00:00:00Z")
        );
        assert_eq!(config.history.len(), 1);
        let period = &config.history[0];
        let cycle = period.billing_cycle.as_ref().unwrap();
        assert_eq!(cycle.year, 2025);
        assert_eq!(cycle.month, 3);
        assert_eq!(period.included_used.as_ref().unwrap().val, 1800);
        assert_eq!(period.total_used.as_ref().unwrap().val, 1800);
    }

    #[test]
    fn billing_unified_log_ctx_includes_credits_and_collapses_history() {
        let resp = BillingConfigResponse {
            config: Some(BillingConfig {
                credit_usage_percent: Some(42.5),
                current_period: Some(UsagePeriod {
                    period_type: Some("USAGE_PERIOD_TYPE_WEEKLY".into()),
                    start: Some("2025-04-01T00:00:00Z".into()),
                    end: Some("2025-04-08T00:00:00Z".into()),
                }),
                usage_windows: Vec::new(),
                monthly_limit: Some(Cent { val: 2000 }),
                used: Some(Cent { val: 850 }),
                on_demand_cap: Some(Cent { val: 500 }),
                on_demand_used: Some(Cent { val: 0 }),
                prepaid_balance: Some(Cent { val: 100 }),
                is_unified_billing_user: Some(true),
                billing_period_start: None,
                billing_period_end: None,
                history: vec![
                    BillingPeriodUsage {
                        billing_cycle: Some(BillingCycle {
                            year: 2025,
                            month: 2,
                        }),
                        included_used: Some(Cent { val: 1000 }),
                        on_demand_used: Some(Cent { val: 0 }),
                        total_used: Some(Cent { val: 1000 }),
                    },
                    BillingPeriodUsage {
                        billing_cycle: Some(BillingCycle {
                            year: 2025,
                            month: 3,
                        }),
                        included_used: Some(Cent { val: 1800 }),
                        on_demand_used: Some(Cent { val: 0 }),
                        total_used: Some(Cent { val: 1800 }),
                    },
                ],
            }),
            on_demand_enabled: Some(true),
            subscription_tier: Some("SuperGrok".into()),
        };
        let ctx = billing_unified_log_ctx(&resp);
        assert_eq!(ctx["onDemandEnabled"], true);
        assert_eq!(ctx["subscriptionTier"], "SuperGrok");
        let config = ctx["config"].as_object().expect("config object");
        assert!(
            config.get("history").is_none(),
            "full history must be collapsed"
        );
        assert_eq!(config["historyLen"], 2);
        assert_eq!(
            config["latestHistory"]["billingCycle"]["month"], 3,
            "latest history period retained"
        );
        assert_eq!(config["creditUsagePercent"], 42.5);
        assert_eq!(config["prepaidBalance"]["val"], 100);
    }

    #[test]
    fn billing_config_response_roundtrips_through_json() {
        let config = BillingConfig {
            credit_usage_percent: None,
            current_period: None,
            usage_windows: vec![UsageWindow {
                period_type: "CHUTES_USAGE_PERIOD_FOUR_HOUR".into(),
                usage_percent: 18.0,
                reset_at: Some("2025-04-01T04:00:00Z".into()),
            }],
            monthly_limit: Some(Cent { val: 5000 }),
            used: Some(Cent { val: 123 }),
            on_demand_cap: Some(Cent { val: 0 }),
            on_demand_used: Some(Cent { val: 50 }),
            prepaid_balance: Some(Cent { val: 750 }),
            is_unified_billing_user: None,
            billing_period_start: Some("2025-04-01T00:00:00Z".to_string()),
            billing_period_end: Some("2025-05-01T00:00:00Z".to_string()),
            history: vec![BillingPeriodUsage {
                billing_cycle: Some(BillingCycle {
                    year: 2025,
                    month: 3,
                }),
                included_used: Some(Cent { val: 4500 }),
                on_demand_used: Some(Cent { val: 100 }),
                total_used: Some(Cent { val: 4600 }),
            }],
        };
        let resp = BillingConfigResponse {
            config: Some(config),
            on_demand_enabled: None,
            subscription_tier: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        let roundtripped: BillingConfigResponse = serde_json::from_value(json).unwrap();
        let rt_config = roundtripped.config.unwrap();
        assert_eq!(rt_config.monthly_limit.unwrap().val, 5000);
        assert_eq!(rt_config.used.unwrap().val, 123);
        assert_eq!(rt_config.prepaid_balance.unwrap().val, 750);
        assert_eq!(rt_config.usage_windows.len(), 1);
        assert_eq!(
            rt_config.usage_windows[0].period_type,
            "CHUTES_USAGE_PERIOD_FOUR_HOUR"
        );
        assert_eq!(rt_config.history.len(), 1);
    }

    #[test]
    fn billing_config_response_handles_null_config() {
        let json = serde_json::json!({"config": null});
        let resp: BillingConfigResponse = serde_json::from_value(json).unwrap();
        assert!(resp.config.is_none());
    }

    #[test]
    fn billing_config_response_handles_empty_history() {
        let json = serde_json::json!({
            "config": {
                "monthlyLimit": {"val": 1000},
                "used": {"val": 0}
            }
        });
        let resp: BillingConfigResponse = serde_json::from_value(json).unwrap();
        let config = resp.config.unwrap();
        assert_eq!(config.monthly_limit.unwrap().val, 1000);
        assert!(config.history.is_empty());
    }

    #[test]
    fn billing_config_serializes_camel_case() {
        let config = BillingConfig {
            credit_usage_percent: None,
            current_period: None,
            usage_windows: Vec::new(),
            monthly_limit: Some(Cent { val: 100 }),
            used: None,
            on_demand_cap: None,
            on_demand_used: None,
            prepaid_balance: None,
            is_unified_billing_user: None,
            billing_period_start: None,
            billing_period_end: None,
            history: vec![],
        };
        let json = serde_json::to_value(&config).unwrap();
        assert!(json.get("monthlyLimit").is_some());
        // Fields with None are skipped
        assert!(json.get("creditUsagePercent").is_none());
        assert!(json.get("currentPeriod").is_none());
        assert!(json.get("usageWindows").is_none());
        assert!(json.get("used").is_none());
        assert!(json.get("onDemandCap").is_none());
        assert!(json.get("onDemandUsed").is_none());
        assert!(json.get("prepaidBalance").is_none());
        assert!(json.get("billingPeriodStart").is_none());
        // Empty history is skipped
        assert!(json.get("history").is_none());
    }

    #[test]
    fn billing_config_deserializes_credits_config_shape() {
        // Newer `GetGrokCreditsConfig` response: percentage-based usage,
        // a typed current period, and history keyed by `period`.
        let json = serde_json::json!({
            "config": {
                "creditUsagePercent": 42.5,
                "currentPeriod": {
                    "type": "USAGE_PERIOD_TYPE_WEEKLY",
                    "start": "2026-06-01T00:00:00Z",
                    "end": "2026-06-08T00:00:00Z"
                },
                "onDemandCap": {"val": 5000},
                "onDemandUsed": {"val": 300},
                "prepaidBalance": {"val": 1250},
                "isUnifiedBillingUser": true,
                "productUsage": [
                    {"product": "PRODUCT_CHUTES_BUILD_BUILD", "usagePercent": 61.2}
                ],
                "history": [
                    {
                        "period": {
                            "type": "USAGE_PERIOD_TYPE_WEEKLY",
                            "start": "2026-05-25T00:00:00Z",
                            "end": "2026-06-01T00:00:00Z"
                        },
                        "onDemandUsed": {"val": 120}
                    }
                ]
            }
        });
        let resp: BillingConfigResponse = serde_json::from_value(json).unwrap();
        let config = resp.config.unwrap();
        assert_eq!(config.credit_usage_percent, Some(42.5));
        let period = config.current_period.as_ref().unwrap();
        assert_eq!(
            period.period_type.as_deref(),
            Some("USAGE_PERIOD_TYPE_WEEKLY")
        );
        assert_eq!(period.end.as_deref(), Some("2026-06-08T00:00:00Z"));
        // Deprecated fields are absent in the credits shape.
        assert!(config.monthly_limit.is_none());
        assert!(config.billing_period_end.is_none());
        assert_eq!(config.on_demand_cap.unwrap().val, 5000);
        assert_eq!(config.on_demand_used.unwrap().val, 300);
        // Bought (prepaid) credit balance is parsed from the credits config.
        assert_eq!(config.prepaid_balance.unwrap().val, 1250);
        assert_eq!(config.is_unified_billing_user, Some(true));
        // productUsage is still unused by the CLI billing surface.
        assert_eq!(config.history.len(), 1);
        assert_eq!(config.history[0].on_demand_used.as_ref().unwrap().val, 120);
    }

    #[test]
    fn cent_serializes_as_val_field() {
        let c = Cent { val: 4299 };
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json, serde_json::json!({"val": 4299}));
    }
}
