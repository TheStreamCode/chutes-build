//! Human-facing summary of a [`ChutesAccountClient`](crate::account::ChutesAccountClient) snapshot.
//!
//! The account endpoints return heterogeneous shapes per tier and quota kind
//! (aggregate vs. per-chute maps, flat vs. nested windows), so every accessor
//! below probes several key spellings and degrades to `None` instead of
//! failing. This mirrors the `chutes-usage-vscode` normalization, which is
//! the reference UI for the same data.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UsageUnit {
    Usd,
    Requests,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UsageWindowKind {
    BillingCycle,
    Rolling4h,
    DailyRequests,
    Weekly,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub id: String,
    pub kind: UsageWindowKind,
    pub label: String,
    pub unit: UsageUnit,
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub remaining: Option<f64>,
    pub percent_used: Option<f64>,
    pub reset_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaEntry {
    pub model_label: String,
    pub quota: Option<f64>,
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanInfo {
    pub plan_name: Option<String>,
    pub monthly_price_usd: Option<f64>,
    pub monthly_cap_usd: Option<f64>,
    pub four_hour_cap_usd: Option<f64>,
    pub daily_request_limit: Option<f64>,
    pub payg_discount_percent: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanLimitEntry {
    pub name: String,
    pub price_label: String,
    pub monthly_cap_label: String,
    pub daily_request_limit_label: String,
    pub four_hour_cap_label: String,
    pub payg_discount_label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub windows: Vec<UsageWindow>,
    pub quotas: Vec<QuotaEntry>,
    pub plan: Option<PlanInfo>,
    pub plan_limits: Vec<PlanLimitEntry>,
    pub payg_credit_usd: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatusBarSummary {
    pub text: String,
    pub severity: StatusSeverity,
}

/// Summarize a `usage_snapshot` payload plus an optional `/users/me`
/// profile into UI-ready windows, plan, and credit.
pub fn summarize_snapshot(
    snapshot: &serde_json::Value,
    profile: &serde_json::Value,
) -> AccountSummary {
    let usage_source = unwrap_usage_payload(snapshot);
    let quotas = normalize_quotas(snapshot.get("quotas").unwrap_or(&serde_json::Value::Null));
    let quota_usage = normalize_quota_usage(snapshot.get("quota_usage"));
    let model_stats = normalize_invocation_stats(snapshot.get("model_stats"));
    let windows = normalize_usage_windows(&usage_source, &quotas, &quota_usage, &model_stats);
    let plan = normalize_plan(&usage_source, &windows);
    AccountSummary {
        windows,
        quotas,
        plan_limits: normalize_plan_limits(plan.as_ref()),
        plan,
        payg_credit_usd: normalize_payg_credit(profile),
    }
}

/// Compact one-line status (`Chutes 4h $1.23/$5`) plus severity, for the
/// TUI status line. Picks the worst window by percent used.
pub fn status_bar_compact(summary: &AccountSummary) -> StatusBarSummary {
    let mut worst: Option<&UsageWindow> = None;
    for window in &summary.windows {
        let Some(percent) = window.percent_used else {
            continue;
        };
        let is_worse = worst.map(|w| w.percent_used.unwrap_or(0.0)).unwrap_or(0.0);
        if worst.is_none() || percent > is_worse {
            worst = Some(window);
        }
    }
    let Some(worst) = worst else {
        return StatusBarSummary {
            text: "Chutes".to_string(),
            severity: StatusSeverity::Info,
        };
    };
    let text = format!(
        "Chutes {} {}",
        short_kind_label(worst.kind),
        format_compact_value(worst)
    )
    .trim()
    .to_string();
    let severity = match worst.percent_used {
        Some(p) if p >= 90.0 => StatusSeverity::Warning,
        _ => StatusSeverity::Info,
    };
    StatusBarSummary { text, severity }
}

fn short_kind_label(kind: UsageWindowKind) -> &'static str {
    match kind {
        UsageWindowKind::BillingCycle => "mo",
        UsageWindowKind::Rolling4h => "4h",
        UsageWindowKind::DailyRequests => "day",
        UsageWindowKind::Weekly => "wk",
    }
}

fn format_compact_value(window: &UsageWindow) -> String {
    match window.unit {
        UsageUnit::Requests => {
            let used = window
                .used
                .map_or("--".to_string(), |v| format!("{}", v.round()));
            let limit = match window.limit {
                None => "--".to_string(),
                Some(0.0) => "∞".to_string(),
                Some(v) => format!("{}", v.round()),
            };
            format!("{used}/{limit}")
        }
        UsageUnit::Usd => {
            let used = window.used.map_or("--".to_string(), |v| format!("${v:.2}"));
            let limit = window
                .limit
                .map_or("--".to_string(), |v| format!("${v:.0}"));
            format!("{used}/{limit}")
        }
    }
}

fn unwrap_usage_payload(snapshot: &serde_json::Value) -> serde_json::Value {
    snapshot
        .get("subscription_usage")
        .cloned()
        .unwrap_or_else(|| snapshot.clone())
}

fn as_number(value: Option<&serde_json::Value>) -> Option<f64> {
    let value = value?;
    if let Some(number) = value.as_f64() {
        return number.is_finite().then_some(number);
    }
    value
        .as_str()
        .and_then(|text| text.trim().parse::<f64>().ok())
        .filter(|number| number.is_finite())
}

fn as_string(value: Option<&serde_json::Value>) -> Option<String> {
    value?.as_str().map(str::to_owned)
}

fn compute_remaining(used: Option<f64>, limit: Option<f64>) -> Option<f64> {
    match (used, limit) {
        (Some(used), Some(limit)) => Some((limit - used).max(0.0)),
        _ => None,
    }
}

fn compute_percent(used: Option<f64>, limit: Option<f64>) -> Option<f64> {
    match (used, limit) {
        (Some(used), Some(limit)) if limit > 0.0 => Some(used / limit * 100.0),
        _ => None,
    }
}

struct WindowCandidate {
    keys: &'static [&'static str],
    kind: UsageWindowKind,
    unit: UsageUnit,
    label: &'static str,
    used_keys: &'static [&'static str],
    limit_keys: &'static [&'static str],
    remaining_keys: &'static [&'static str],
    reset_keys: &'static [&'static str],
    nested_used_keys: &'static [&'static str],
    nested_limit_keys: &'static [&'static str],
    nested_remaining_keys: &'static [&'static str],
    nested_reset_keys: &'static [&'static str],
}

const CANDIDATES: &[WindowCandidate] = &[
    WindowCandidate {
        keys: &[
            "billing_cycle_cap",
            "monthly_cap",
            "monthly_window",
            "billing_cycle",
            "monthly",
        ],
        kind: UsageWindowKind::BillingCycle,
        unit: UsageUnit::Usd,
        label: "Billing Cycle Cap",
        used_keys: &["billing_cycle_used", "monthly_used"],
        limit_keys: &["billing_cycle_limit", "monthly_limit", "monthly_cap_usd"],
        remaining_keys: &["billing_cycle_remaining", "monthly_remaining"],
        reset_keys: &["billing_cycle_reset_label", "monthly_reset_label"],
        nested_used_keys: &["usage", "used"],
        nested_limit_keys: &["cap", "limit"],
        nested_remaining_keys: &["remaining"],
        nested_reset_keys: &["reset_at", "reset_label"],
    },
    WindowCandidate {
        keys: &[
            "four_hour_window",
            "rolling_4h_window",
            "four_hour_cap",
            "rolling_window",
            "four_hour",
        ],
        kind: UsageWindowKind::Rolling4h,
        unit: UsageUnit::Usd,
        label: "4-Hour Window",
        used_keys: &["four_hour_used", "rolling_4h_used"],
        limit_keys: &["four_hour_limit", "rolling_4h_limit", "four_hour_cap_usd"],
        remaining_keys: &["four_hour_remaining", "rolling_4h_remaining"],
        reset_keys: &["four_hour_reset_label", "rolling_4h_reset_label"],
        nested_used_keys: &["usage", "used"],
        nested_limit_keys: &["cap", "limit"],
        nested_remaining_keys: &["remaining"],
        nested_reset_keys: &["reset_at", "reset_label"],
    },
    WindowCandidate {
        keys: &[
            "daily_quota_usage",
            "daily_quota",
            "daily_window",
            "daily_requests",
        ],
        kind: UsageWindowKind::DailyRequests,
        unit: UsageUnit::Requests,
        label: "Daily Quota",
        used_keys: &["daily_used", "daily_quota_used"],
        limit_keys: &["daily_limit", "daily_request_limit", "daily_quota_limit"],
        remaining_keys: &["daily_remaining", "daily_quota_remaining"],
        reset_keys: &["daily_reset_label", "daily_quota_reset_label"],
        nested_used_keys: &["usage", "used"],
        nested_limit_keys: &["cap", "limit", "quota"],
        nested_remaining_keys: &["remaining"],
        nested_reset_keys: &["reset_at", "reset_label"],
    },
    WindowCandidate {
        keys: &["weekly_window", "weekly_cap"],
        kind: UsageWindowKind::Weekly,
        unit: UsageUnit::Usd,
        label: "Weekly Window",
        used_keys: &["weekly_used"],
        limit_keys: &["weekly_limit"],
        remaining_keys: &["weekly_remaining"],
        reset_keys: &["weekly_reset_label"],
        nested_used_keys: &["used"],
        nested_limit_keys: &["limit"],
        nested_remaining_keys: &["remaining"],
        nested_reset_keys: &["reset_label"],
    },
];

fn pick_nested<'a>(
    payload: &'a serde_json::Value,
    keys: &[&'a str],
) -> (Option<&'a str>, Option<&'a serde_json::Value>) {
    for key in keys {
        if let Some(value @ serde_json::Value::Object(_)) = payload.get(*key) {
            return (Some(key), Some(value));
        }
    }
    (None, None)
}

fn pick_number(
    nested: Option<&serde_json::Value>,
    payload: &serde_json::Value,
    keys: &[&str],
    nested_keys: &[&str],
) -> Option<f64> {
    if let Some(nested) = nested {
        for key in nested_keys {
            if let Some(number) = as_number(nested.get(*key)) {
                return Some(number);
            }
        }
    }
    for key in keys {
        if let Some(number) = as_number(payload.get(*key)) {
            return Some(number);
        }
    }
    None
}

fn pick_string(
    nested: Option<&serde_json::Value>,
    payload: &serde_json::Value,
    keys: &[&str],
    nested_keys: &[&str],
) -> Option<String> {
    if let Some(nested) = nested {
        for key in nested_keys {
            if let Some(text) = as_string(nested.get(*key)) {
                return Some(text);
            }
        }
    }
    for key in keys {
        if let Some(text) = as_string(payload.get(*key)) {
            return Some(text);
        }
    }
    None
}

#[derive(Debug, Clone, Copy, Default)]
struct QuotaUsageSummary {
    used: Option<f64>,
    quota: Option<f64>,
    trusted: bool,
}

fn normalize_quota_usage(payload: Option<&serde_json::Value>) -> Option<QuotaUsageSummary> {
    let payload = payload?;
    if payload.is_array() {
        return None;
    }
    let object = payload.as_object()?;
    let direct_used = as_number(object.get("used"));
    let direct_quota = as_number(object.get("quota"));
    if direct_used.is_some() || direct_quota.is_some() {
        return Some(QuotaUsageSummary {
            used: direct_used,
            quota: direct_quota,
            trusted: true,
        });
    }
    let mut used_total: Option<f64> = None;
    let mut quota_total: Option<f64> = None;
    for value in object.values() {
        let Some(entry) = value.as_object() else {
            continue;
        };
        if let Some(used) = as_number(entry.get("used")) {
            used_total = Some(used_total.unwrap_or(0.0) + used);
        }
        if let Some(quota) = as_number(entry.get("quota")) {
            quota_total = Some(quota_total.unwrap_or(0.0) + quota);
        }
    }
    if used_total.is_some() || quota_total.is_some() {
        Some(QuotaUsageSummary {
            used: used_total,
            quota: quota_total,
            trusted: true,
        })
    } else {
        None
    }
}

fn normalize_invocation_stats(payload: Option<&serde_json::Value>) -> Option<f64> {
    let entries = payload?.as_array()?;
    let mut total = 0.0;
    let mut has_stats = false;
    for entry in entries {
        if let Some(requests) = entry.get("total_requests").and_then(|v| as_number(Some(v))) {
            total += requests;
            has_stats = true;
        }
    }
    has_stats.then_some(total)
}

fn normalize_quotas(payload: &serde_json::Value) -> Vec<QuotaEntry> {
    let items = if let Some(items) = payload.as_array() {
        items.clone()
    } else if let Some(items) = payload.get("items").and_then(|v| v.as_array()) {
        items.clone()
    } else if let Some(items) = payload.get("quotas").and_then(|v| v.as_array()) {
        items.clone()
    } else {
        Vec::new()
    };
    items
        .iter()
        .filter_map(|item| item.as_object())
        .map(|item| QuotaEntry {
            model_label: as_string(item.get("model"))
                .or_else(|| as_string(item.get("model_label")))
                .or_else(|| as_string(item.get("label")))
                .or_else(|| as_string(item.get("name")))
                .unwrap_or_else(|| "All Models".to_string()),
            quota: as_number(item.get("quota")).or_else(|| as_number(item.get("limit"))),
            last_updated: as_string(item.get("last_updated"))
                .or_else(|| as_string(item.get("updated_at"))),
        })
        .collect()
}

fn normalize_usage_windows(
    payload: &serde_json::Value,
    quotas: &[QuotaEntry],
    quota_usage: &Option<QuotaUsageSummary>,
    invocation_total_requests: &Option<f64>,
) -> Vec<UsageWindow> {
    let live_daily = build_daily_quota_window(quotas, quota_usage, invocation_total_requests);
    let mut windows: Vec<UsageWindow> = CANDIDATES
        .iter()
        .filter_map(|candidate| {
            let (matched_key, nested) = pick_nested(payload, candidate.keys);
            let used = pick_number(
                nested,
                payload,
                candidate.used_keys,
                candidate.nested_used_keys,
            );
            let limit = pick_number(
                nested,
                payload,
                candidate.limit_keys,
                candidate.nested_limit_keys,
            );
            let remaining = pick_number(
                nested,
                payload,
                candidate.remaining_keys,
                candidate.nested_remaining_keys,
            )
            .or_else(|| compute_remaining(used, limit));
            let reset_label = pick_string(
                nested,
                payload,
                candidate.reset_keys,
                candidate.nested_reset_keys,
            );
            let label = pick_string(nested, payload, &[], &["label"])
                .unwrap_or_else(|| candidate.label.to_string());
            if nested.is_none() && used.is_none() && limit.is_none() && remaining.is_none() {
                return None;
            }
            Some(UsageWindow {
                id: matched_key.unwrap_or(candidate.keys[0]).to_string(),
                kind: candidate.kind,
                label,
                unit: candidate.unit,
                used,
                limit,
                remaining,
                percent_used: compute_percent(used, limit),
                reset_label,
                status: None,
                data_source: None,
            })
        })
        .filter(|window| !(window.kind == UsageWindowKind::DailyRequests && live_daily.is_some()))
        .collect();
    if let Some(daily) = live_daily {
        windows.push(daily);
    }
    let mut seen = std::collections::HashSet::new();
    windows.retain(|window| seen.insert(window.kind as u8));
    windows
}

fn build_daily_quota_window(
    quotas: &[QuotaEntry],
    quota_usage: &Option<QuotaUsageSummary>,
    invocation_total_requests: &Option<f64>,
) -> Option<UsageWindow> {
    let mut total_quota: Option<f64> = None;
    for entry in quotas {
        if let Some(quota) = entry.quota {
            total_quota = Some(total_quota.unwrap_or(0.0) + quota);
        }
    }
    total_quota?;
    let limit = quota_usage
        .and_then(|summary| summary.quota)
        .or(total_quota);
    let is_unlimited = limit == Some(0.0);
    let live_total = invocation_total_requests.unwrap_or(0.0);
    let is_stale = quota_usage.map(|summary| summary.used) == Some(Some(0.0)) && live_total > 0.0;
    let used = if is_stale {
        None
    } else {
        quota_usage.and_then(|summary| summary.trusted.then_some(summary.used).flatten())
    };
    let remaining = if is_unlimited {
        None
    } else {
        match (used, limit) {
            (Some(used), Some(limit)) => compute_remaining(Some(used), Some(limit)),
            _ => None,
        }
    };
    let status = if is_stale {
        "stale"
    } else if used.is_some() {
        "trusted"
    } else {
        "unknown"
    };
    Some(UsageWindow {
        id: "quotas-daily".to_string(),
        kind: UsageWindowKind::DailyRequests,
        label: "Daily Quota".to_string(),
        unit: UsageUnit::Requests,
        used,
        limit,
        remaining,
        percent_used: compute_percent(used, limit),
        reset_label: is_stale.then(|| "Possible sync delay".to_string()),
        status: Some(status.to_string()),
        data_source: Some("quota-usage".to_string()),
    })
}

fn normalize_payg_credit(profile: &serde_json::Value) -> Option<f64> {
    let object = profile.as_object()?;
    as_number(object.get("balance"))
        .or_else(|| as_number(object.get("effective_balance")))
        .or_else(|| {
            object
                .get("current_balance")
                .and_then(|v| v.as_object())
                .and_then(|inner| as_number(inner.get("effective_balance")))
        })
        .or_else(|| as_number(object.get("payment_balance")))
}

fn normalize_plan(payload: &serde_json::Value, windows: &[UsageWindow]) -> Option<PlanInfo> {
    let plan_object = payload.get("plan").and_then(|v| v.as_object());
    let empty = serde_json::Map::new();
    let source = plan_object.unwrap_or(&empty);
    let object = payload.as_object()?;
    let billing_window = windows
        .iter()
        .find(|w| w.kind == UsageWindowKind::BillingCycle);
    let rolling_window = windows
        .iter()
        .find(|w| w.kind == UsageWindowKind::Rolling4h);
    let daily_window = windows
        .iter()
        .find(|w| w.kind == UsageWindowKind::DailyRequests);
    let billing_cap = payload.get("billing_cycle_cap").and_then(|v| v.as_object());
    let four_hour_cap = payload.get("four_hour_window").and_then(|v| v.as_object());
    let daily_quota = payload.get("daily_quota_usage").and_then(|v| v.as_object());

    let get = |map: &serde_json::Map<String, serde_json::Value>, key: &str| {
        map.get(key).and_then(|v| as_number(Some(v)))
    };
    let plan_name = source
        .get("name")
        .and_then(|v| as_string(Some(v)))
        .or_else(|| as_string(object.get("plan_name")))
        .or_else(|| derive_plan_name(object));
    let monthly_price_usd =
        get(source, "monthly_price").or_else(|| as_number(object.get("monthly_price")));
    let monthly_cap_usd = get(source, "monthly_cap_usd")
        .or_else(|| as_number(object.get("monthly_cap_usd")))
        .or_else(|| as_number(object.get("billing_cycle_limit")))
        .or_else(|| {
            object
                .get("monthly")
                .and_then(|v| v.as_object())
                .and_then(|inner| as_number(inner.get("cap")))
        })
        .or_else(|| billing_cap.and_then(|inner| as_number(inner.get("limit"))))
        .or_else(|| billing_window.and_then(|w| w.limit));
    let four_hour_cap_usd = get(source, "four_hour_cap_usd")
        .or_else(|| as_number(object.get("four_hour_cap_usd")))
        .or_else(|| as_number(object.get("four_hour_limit")))
        .or_else(|| {
            object
                .get("four_hour")
                .and_then(|v| v.as_object())
                .and_then(|inner| as_number(inner.get("cap")))
        })
        .or_else(|| four_hour_cap.and_then(|inner| as_number(inner.get("limit"))))
        .or_else(|| rolling_window.and_then(|w| w.limit));
    let daily_request_limit = get(source, "daily_request_limit")
        .or_else(|| as_number(object.get("daily_request_limit")))
        .or_else(|| as_number(object.get("daily_limit")))
        .or_else(|| daily_quota.and_then(|inner| as_number(inner.get("limit"))))
        .or_else(|| daily_window.and_then(|w| w.limit));
    let payg_discount_percent = get(source, "payg_discount_percent")
        .or_else(|| as_number(object.get("payg_discount_percent")));

    if plan_name.is_none()
        && monthly_price_usd.is_none()
        && monthly_cap_usd.is_none()
        && four_hour_cap_usd.is_none()
        && daily_request_limit.is_none()
        && payg_discount_percent.is_none()
    {
        return None;
    }
    Some(PlanInfo {
        plan_name,
        monthly_price_usd,
        monthly_cap_usd,
        four_hour_cap_usd,
        daily_request_limit,
        payg_discount_percent,
    })
}

fn derive_plan_name(payload: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let subscription = payload.get("subscription").and_then(|v| v.as_bool());
    let custom = payload.get("custom").and_then(|v| v.as_bool());
    let monthly_price = payload
        .get("monthly_price")
        .and_then(|v| as_number(Some(v)));
    if subscription == Some(false) {
        return Some("Free tier".to_string());
    }
    if let Some(price) = monthly_price {
        if price == 10.0 {
            return Some("Plus".to_string());
        }
        if price == 20.0 {
            return Some("Pro".to_string());
        }
    }
    if subscription == Some(true) && custom == Some(false) {
        return Some("Paid tier".to_string());
    }
    if custom == Some(true) {
        return Some("Custom".to_string());
    }
    None
}

fn normalize_plan_limits(plan: Option<&PlanInfo>) -> Vec<PlanLimitEntry> {
    let references = [("Plus", 10.0, 6.0), ("Pro", 20.0, 10.0)];
    let mut entries: Vec<PlanLimitEntry> = references
        .iter()
        .map(|(name, price, discount)| {
            let is_current = plan
                .and_then(|plan| plan.plan_name.as_deref())
                .is_some_and(|current| current.eq_ignore_ascii_case(name));
            to_plan_limit_entry(
                name,
                if is_current { plan } else { None },
                Some(*price),
                Some(*discount),
            )
        })
        .collect();
    if let Some(current) = plan.and_then(|plan| plan.plan_name.as_deref())
        && !references
            .iter()
            .any(|(name, _, _)| name.eq_ignore_ascii_case(current))
    {
        entries.insert(0, to_plan_limit_entry(current, plan, None, None));
    }
    entries
}

fn to_plan_limit_entry(
    name: &str,
    plan: Option<&PlanInfo>,
    reference_price: Option<f64>,
    reference_discount: Option<f64>,
) -> PlanLimitEntry {
    let monthly_price = plan
        .and_then(|plan| plan.monthly_price_usd)
        .or(reference_price);
    let payg_discount = plan
        .and_then(|plan| plan.payg_discount_percent)
        .or(reference_discount);
    PlanLimitEntry {
        name: name.to_string(),
        price_label: monthly_price.map_or("--".to_string(), |v| format!("{}/mo", format_usd(v))),
        monthly_cap_label: plan
            .and_then(|plan| plan.monthly_cap_usd)
            .map_or("--".to_string(), format_usd),
        daily_request_limit_label: plan
            .and_then(|plan| plan.daily_request_limit)
            .map_or("--".to_string(), |v| format!("{}", v.round())),
        four_hour_cap_label: plan
            .and_then(|plan| plan.four_hour_cap_usd)
            .map_or("--".to_string(), format_usd),
        payg_discount_label: payg_discount.map_or("--".to_string(), |v| format!("{}%", v.round())),
    }
}

fn format_usd(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("${value:.0}")
    } else {
        format!("${value:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_fixture() -> serde_json::Value {
        serde_json::json!({
            "subscription_usage": {
                "plan": {"name": "Pro", "monthly_price": 20},
                "billing_cycle_cap": {"usage": 12.5, "cap": 100, "reset_label": "Nov 1"},
                "four_hour_window": {"used": "4.25", "limit": 10},
                "monthly_price": 20,
                "payg_discount_percent": 10
            },
            "quotas": [{"model": "model-router", "quota": 5000, "updated_at": "2026-10-15"}],
            "quota_usage": {"used": 120, "quota": 5000},
            "model_stats": [{"total_requests": 120}]
        })
    }

    #[test]
    fn summary_names_windows_plan_and_credit() {
        let profile = serde_json::json!({"balance": 7.5});
        let summary = summarize_snapshot(&snapshot_fixture(), &profile);
        assert_eq!(summary.windows.len(), 3);
        let billing = summary
            .windows
            .iter()
            .find(|w| w.kind == UsageWindowKind::BillingCycle)
            .expect("billing window");
        assert_eq!(billing.used, Some(12.5));
        assert_eq!(billing.limit, Some(100.0));
        assert_eq!(billing.remaining, Some(87.5));
        assert_eq!(billing.percent_used, Some(12.5));
        assert_eq!(billing.reset_label.as_deref(), Some("Nov 1"));
        let rolling = summary
            .windows
            .iter()
            .find(|w| w.kind == UsageWindowKind::Rolling4h)
            .expect("rolling window");
        assert_eq!(rolling.used, Some(4.25));
        let daily = summary
            .windows
            .iter()
            .find(|w| w.kind == UsageWindowKind::DailyRequests)
            .expect("daily window");
        assert_eq!(daily.used, Some(120.0));
        assert_eq!(daily.limit, Some(5000.0));
        assert_eq!(daily.status.as_deref(), Some("trusted"));
        let plan = summary.plan.expect("plan");
        assert_eq!(plan.plan_name.as_deref(), Some("Pro"));
        assert_eq!(plan.monthly_price_usd, Some(20.0));
        assert_eq!(plan.payg_discount_percent, Some(10.0));
        assert_eq!(summary.payg_credit_usd, Some(7.5));
        assert_eq!(summary.quotas.len(), 1);
        assert_eq!(summary.quotas[0].model_label, "model-router");
    }

    #[test]
    fn stale_quota_usage_yields_unknown_daily_window() {
        let mut snapshot = snapshot_fixture();
        snapshot["quota_usage"] = serde_json::json!({"used": 0, "quota": 5000});
        let summary = summarize_snapshot(&snapshot, &serde_json::Value::Null);
        let daily = summary
            .windows
            .iter()
            .find(|w| w.kind == UsageWindowKind::DailyRequests)
            .expect("daily window");
        assert_eq!(daily.used, None);
        assert_eq!(daily.status.as_deref(), Some("stale"));
        assert_eq!(daily.reset_label.as_deref(), Some("Possible sync delay"));
    }

    #[test]
    fn plan_derives_from_price_and_flags() {
        let snapshot = serde_json::json!({
            "subscription_usage": {"monthly_price": 10, "subscription": true, "custom": false},
            "quotas": [],
            "quota_usage": serde_json::Value::Null,
            "model_stats": serde_json::Value::Null,
        });
        let summary = summarize_snapshot(&snapshot, &serde_json::Value::Null);
        assert_eq!(
            summary.plan.expect("plan").plan_name.as_deref(),
            Some("Plus")
        );
        let free = serde_json::json!({
            "subscription_usage": {"subscription": false},
            "quotas": [],
            "quota_usage": serde_json::Value::Null,
            "model_stats": serde_json::Value::Null,
        });
        let summary = summarize_snapshot(&free, &serde_json::Value::Null);
        assert_eq!(
            summary.plan.expect("plan").plan_name.as_deref(),
            Some("Free tier")
        );
    }

    #[test]
    fn empty_snapshot_summarizes_to_nothing() {
        let summary = summarize_snapshot(&serde_json::Value::Null, &serde_json::Value::Null);
        assert!(summary.windows.is_empty());
        assert!(summary.quotas.is_empty());
        assert!(summary.plan.is_none());
        assert_eq!(summary.payg_credit_usd, None);
        assert_eq!(summary.plan_limits.len(), 2);
    }

    #[test]
    fn status_bar_picks_worst_window_and_warns_past_ninety() {
        let summary = summarize_snapshot(&snapshot_fixture(), &serde_json::Value::Null);
        let status = status_bar_compact(&summary);
        assert_eq!(status.severity, StatusSeverity::Info);
        assert!(status.text.starts_with("Chutes "), "{status:?}");
        let hot = serde_json::json!({
            "subscription_usage": {"four_hour_window": {"used": 9.5, "limit": 10}},
            "quotas": [],
            "quota_usage": serde_json::Value::Null,
            "model_stats": serde_json::Value::Null,
        });
        let summary = summarize_snapshot(&hot, &serde_json::Value::Null);
        let status = status_bar_compact(&summary);
        assert_eq!(status.severity, StatusSeverity::Warning);
        assert_eq!(status.text, "Chutes 4h $9.50/$10");
    }
}
