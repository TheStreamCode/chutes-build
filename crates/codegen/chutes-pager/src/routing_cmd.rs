//! `chutes-build routing`: saved Chutes model-routing pool and live TTFT/TPS.

use anyhow::{Context, Result};
use chutes_build_core::account::ChutesAccountClient;
use chutes_build_core::routing_pools::{self, RoutingStat, RoutingView};

#[derive(Debug, clap::Args, Clone)]
pub struct RoutingArgs {
    /// Print the routing view as JSON.
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: RoutingArgs) -> Result<()> {
    let client = ChutesAccountClient::from_env().context("Cannot load Chutes routing data")?;
    let view = routing_pools::fetch_view(&client)
        .await
        .map_err(|error| anyhow::anyhow!("Chutes routing request failed: {error}"))?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        print!("{}", format_human(&view));
    }
    Ok(())
}

/// Human-readable routing view, shared with the TUI `/routing` command.
pub fn format_human(view: &RoutingView) -> String {
    let mut out = String::from("Chutes Model Routing\n\n");
    if view.aliases.is_empty() {
        out.push_str("Saved pool: not configured\n");
    } else {
        out.push_str("Saved pool\n");
        for alias in &view.aliases {
            out.push_str(&format!(
                "  {}: {} model{}\n",
                alias.alias,
                alias.chute_ids.len(),
                if alias.chute_ids.len() == 1 { "" } else { "s" }
            ));
        }
    }
    out.push('\n');
    out.push_str("Strategies: default (failover), default:latency (fastest TTFT), default:throughput (highest TPS)\n");

    let stats = display_stats(&view.stats);
    if !stats.is_empty() {
        out.push('\n');
        out.push_str("Model latency (live)\n");
        let total = stats.len();
        for stat in stats.into_iter().take(25) {
            let ttft = stat
                .average_ttft_secs
                .map_or("--".to_string(), |value| format!("{value:.2}s"));
            let tps = stat
                .average_tps
                .map_or("--".to_string(), |value| format!("{value:.1} tok/s"));
            let requests = stat
                .total_requests
                .map_or("--".to_string(), |value| value.to_string());
            out.push_str(&format!(
                "  {:<38} TTFT {:>7}  {:>12}  {:>10} req\n",
                stat.name, ttft, tps, requests
            ));
        }
        if total > 25 {
            out.push_str(&format!("  ... {} more models\n", total - 25));
        }
    }
    out
}

/// The stats endpoint returns one row per chute and date, including private
/// chutes and rows with no traffic. Keep public catalogue models (owner/model
/// ids) that actually served requests, one row per model.
fn display_stats<'a>(stats: &'a [RoutingStat]) -> Vec<&'a RoutingStat> {
    use std::collections::HashMap;
    let mut best: HashMap<&str, &RoutingStat> = HashMap::new();
    for stat in stats {
        if !stat.name.contains('/') {
            continue;
        }
        if stat.average_ttft_secs.unwrap_or(0.0) <= 0.0 {
            continue;
        }
        if stat.total_requests.unwrap_or(0) == 0 {
            continue;
        }
        best.entry(stat.name.as_str())
            .and_modify(|current| {
                if stat.total_requests.unwrap_or(0) > current.total_requests.unwrap_or(0) {
                    *current = stat;
                }
            })
            .or_insert(stat);
    }
    let mut stats: Vec<&RoutingStat> = best.into_values().collect();
    stats.sort_by(|left, right| {
        left.average_ttft_secs
            .partial_cmp(&right.average_ttft_secs)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.name.cmp(&right.name))
    });
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use chutes_build_core::routing_pools::{RoutingAlias, RoutingStat};

    fn fixture() -> RoutingView {
        RoutingView {
            aliases: vec![RoutingAlias {
                alias: "default".to_string(),
                chute_ids: vec!["a".to_string(), "b".to_string()],
            }],
            stats: vec![
                RoutingStat {
                    chute_id: "a".to_string(),
                    name: "acme/model-a".to_string(),
                    average_ttft_secs: Some(0.34),
                    average_tps: Some(37.1),
                    total_requests: Some(438_903),
                    date: None,
                },
                RoutingStat {
                    chute_id: "b".to_string(),
                    name: "acme/model-b".to_string(),
                    average_ttft_secs: Some(1.2),
                    average_tps: Some(10.0),
                    total_requests: Some(5),
                    date: None,
                },
            ],
        }
    }

    #[test]
    fn human_format_lists_pool_and_sorts_by_ttft() {
        let text = format_human(&fixture());
        assert!(text.contains("default: 2 models"));
        assert!(text.contains("TTFT"));
        assert!(text.contains("acme/model-a"));
        assert!(text.contains("acme/model-b"));
        let a = text.find("acme/model-a").expect("a");
        let b = text.find("acme/model-b").expect("b");
        assert!(a < b, "fastest model must come first: {text}");
    }

    #[test]
    fn human_format_handles_missing_pool() {
        let text = format_human(&RoutingView::default());
        assert!(text.contains("Saved pool: not configured"));
    }
}
