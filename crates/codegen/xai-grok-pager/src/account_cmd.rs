//! `chutes-build account`: Chutes plan, quotas, usage, and credit for the configured API key.

use anyhow::{Context, Result};
use chutes_build_core::account::ChutesAccountClient;
use chutes_build_core::account_summary::{self, AccountSummary};

#[derive(Debug, clap::Args, Clone)]
pub struct AccountArgs {
    /// Print the account summary as JSON.
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: AccountArgs) -> Result<()> {
    let summary = collect().await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print!("{}", format_human(&summary));
    }
    Ok(())
}

async fn collect() -> Result<AccountSummary> {
    let client =
        ChutesAccountClient::from_env().context("Cannot load Chutes account data")?;
    let snapshot = client
        .usage_snapshot(true)
        .await
        .context("Chutes account request failed")?;
    let profile = client.profile().await;
    Ok(account_summary::summarize_snapshot(&snapshot, &profile))
}

/// Human-readable dashboard. Shared with the TUI `/account` command.
pub fn format_human(summary: &AccountSummary) -> String {
    let mut out = String::from("Chutes account\n\n");
    match summary.plan.as_ref() {
        Some(plan) => {
            out.push_str(&format!("Plan: {}\n", plan.plan_name.as_deref().unwrap_or("Unknown")));
            let mut caps = Vec::new();
            if let Some(price) = plan.monthly_price_usd {
                caps.push(format!("{} /mo", format_usd(price)));
            }
            if let Some(cap) = plan.monthly_cap_usd {
                caps.push(format!("monthly cap {}", format_usd(cap)));
            }
            if let Some(cap) = plan.four_hour_cap_usd {
                caps.push(format!("4-hour cap {}", format_usd(cap)));
            }
            if let Some(limit) = plan.daily_request_limit {
                caps.push(format!("daily requests {}", format_int(limit)));
            }
            if let Some(discount) = plan.payg_discount_percent {
                caps.push(format!("PAYG discount {}%", discount.round()));
            }
            if !caps.is_empty() {
                out.push_str(&format!("  {}\n", caps.join(", ")));
            }
        }
        None => out.push_str("Plan: unknown\n"),
    }
    match summary.payg_credit_usd {
        Some(credit) => out.push_str(&format!("Credit: {} PAYG balance\n", format_usd(credit))),
        None => out.push_str("Credit: unknown\n"),
    }
    out.push('\n');
    if summary.windows.is_empty() {
        out.push_str("Usage: no data\n");
    } else {
        out.push_str("Usage\n");
        for window in &summary.windows {
            out.push_str(&format!("  {:<18} {}\n", window.label, format_window_value(window)));
        }
    }
    if !summary.quotas.is_empty() {
        out.push('\n');
        out.push_str("Quotas\n");
        for quota in &summary.quotas {
            match quota.quota {
                Some(limit) => out.push_str(&format!(
                    "  {}: {}\n",
                    quota.model_label,
                    format_int(limit)
                )),
                None => out.push_str(&format!("  {}: unknown\n", quota.model_label)),
            }
        }
    }
    out
}

fn format_window_value(window: &account_summary::UsageWindow) -> String {
    use chutes_build_core::account_summary::UsageUnit;
    let mut text = match window.unit {
        UsageUnit::Usd => format!(
            "{} / {}",
            window.used.map_or("--".to_string(), format_usd),
            window.limit.map_or("--".to_string(), format_usd)
        ),
        UsageUnit::Requests => format!(
            "{} / {}",
            window.used.map_or("--".to_string(), format_int),
            window.limit.map_or("--".to_string(), format_int)
        ),
    };
    if let Some(percent) = window.percent_used {
        text.push_str(&format!(" ({percent:.0}%)"));
    }
    if window.unit == UsageUnit::Requests {
        text.push_str(" requests");
    }
    if let Some(reset) = window.reset_label.as_deref() {
        text.push_str(&format!(" · resets {reset}"));
    }
    if window.status.as_deref() == Some("stale") {
        text.push_str(" · stale");
    }
    text
}

fn format_usd(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("${value:.0}")
    } else {
        format!("${value:.2}")
    }
}

fn format_int(value: f64) -> String {
    value.round().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chutes_build_core::account_summary::{
        PlanInfo, QuotaEntry, UsageUnit, UsageWindow, UsageWindowKind,
    };

    fn summary_fixture() -> AccountSummary {
        AccountSummary {
            windows: vec![
                UsageWindow {
                    id: "billing_cycle_cap".to_string(),
                    kind: UsageWindowKind::BillingCycle,
                    label: "Billing Cycle Cap".to_string(),
                    unit: UsageUnit::Usd,
                    used: Some(12.5),
                    limit: Some(100.0),
                    remaining: Some(87.5),
                    percent_used: Some(12.5),
                    reset_label: Some("Nov 1".to_string()),
                    status: None,
                    data_source: None,
                },
                UsageWindow {
                    id: "quotas-daily".to_string(),
                    kind: UsageWindowKind::DailyRequests,
                    label: "Daily Quota".to_string(),
                    unit: UsageUnit::Requests,
                    used: Some(120.0),
                    limit: Some(5000.0),
                    remaining: Some(4880.0),
                    percent_used: Some(2.4),
                    reset_label: None,
                    status: Some("trusted".to_string()),
                    data_source: Some("quota-usage".to_string()),
                },
            ],
            quotas: vec![QuotaEntry {
                model_label: "model-router".to_string(),
                quota: Some(5000.0),
                last_updated: None,
            }],
            plan: Some(PlanInfo {
                plan_name: Some("Pro".to_string()),
                monthly_price_usd: Some(20.0),
                monthly_cap_usd: Some(100.0),
                four_hour_cap_usd: Some(10.0),
                daily_request_limit: Some(5000.0),
                payg_discount_percent: Some(10.0),
            }),
            plan_limits: Vec::new(),
            payg_credit_usd: Some(7.5),
        }
    }

    #[test]
    fn human_dashboard_names_plan_credit_usage_and_quotas() {
        let text = format_human(&summary_fixture());
        for token in [
            "Plan: Pro",
            "$20 /mo",
            "10%",
            "Credit: $7.50",
            "Billing Cycle Cap",
            "$12.50 / $100",
            "Daily Quota",
            "120 / 5000",
            "model-router: 5000",
        ] {
            assert!(text.contains(token), "{token} missing:\n{text}");
        }
    }

    #[test]
    fn human_dashboard_degrades_without_data() {
        let text = format_human(&AccountSummary {
            windows: Vec::new(),
            quotas: Vec::new(),
            plan: None,
            plan_limits: Vec::new(),
            payg_credit_usd: None,
        });
        for token in ["Plan: unknown", "Credit: unknown", "Usage: no data"] {
            assert!(text.contains(token), "{token} missing:\n{text}");
        }
    }
}
