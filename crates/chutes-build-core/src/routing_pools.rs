//! Read-only Chutes model-routing pools and live model statistics.
//!
//! The Chutes web app lets an account save a pool of chutes under the `default`
//! alias and pick a strategy per request (`default`, `default:latency`,
//! `default:throughput`). This module exposes that pool and the provider's live
//! TTFT/TPS table so the CLI can show them, and lets Auto prefer the saved pool
//! when one exists.

use std::sync::{LazyLock, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::account::ChutesAccountClient;

const ALIAS_CACHE_TTL: Duration = Duration::from_secs(60);
const STATS_CACHE_TTL: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingAlias {
    pub alias: String,
    pub chute_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingStat {
    pub chute_id: String,
    pub name: String,
    pub average_ttft_secs: Option<f64>,
    pub average_tps: Option<f64>,
    pub total_requests: Option<u64>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingView {
    pub aliases: Vec<RoutingAlias>,
    pub stats: Vec<RoutingStat>,
}

struct Cached<T> {
    fetched_at: Instant,
    value: T,
}

static ALIAS_CACHE: LazyLock<RwLock<Option<Cached<Vec<RoutingAlias>>>>> =
    LazyLock::new(|| RwLock::new(None));
static STATS_CACHE: LazyLock<RwLock<Option<Cached<Vec<RoutingStat>>>>> =
    LazyLock::new(|| RwLock::new(None));

/// Parse the `GET /model_aliases/` payload. Accepts the array shape the API
/// returns today and the alias-keyed object shape seen in older deployments.
pub fn parse_aliases(value: &serde_json::Value) -> Vec<RoutingAlias> {
    fn from_record(record: &serde_json::Value) -> Option<RoutingAlias> {
        let obj = record.as_object()?;
        let alias = first_string(obj, &["alias", "name", "id"])?;
        let ids = obj
            .get("chute_ids")
            .or_else(|| obj.get("chuteIds"))
            .or_else(|| obj.get("chutes"))
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        item.as_str()
                            .map(str::to_owned)
                            .or_else(|| {
                                item.get("chute_id")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_owned)
                            })
                            .or_else(|| item.get("id").and_then(|v| v.as_str()).map(str::to_owned))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(RoutingAlias {
            alias,
            chute_ids: ids,
        })
    }

    match value {
        serde_json::Value::Array(items) => items.iter().filter_map(from_record).collect(),
        serde_json::Value::Object(map) => {
            if map.keys().any(|key| key == "alias" || key == "chute_ids") {
                return from_record(value).into_iter().collect();
            }
            map.iter()
                .filter_map(|(alias, ids)| {
                    let ids = ids.as_array()?;
                    Some(RoutingAlias {
                        alias: alias.clone(),
                        chute_ids: ids
                            .iter()
                            .filter_map(|id| id.as_str().map(str::to_owned))
                            .collect(),
                    })
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Parse the `GET /invocations/stats/llm` table. `average_ttft` is seconds and
/// `average_tps` is tokens per second; both are optional because the table is
/// published per chute and some rows omit a field.
pub fn parse_stats(value: &serde_json::Value) -> Vec<RoutingStat> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let obj = item.as_object()?;
            let chute_id = first_string(obj, &["chute_id", "chuteId", "id"])?;
            let name =
                first_string(obj, &["name", "model", "slug"]).unwrap_or_else(|| chute_id.clone());
            Some(RoutingStat {
                chute_id,
                name,
                average_ttft_secs: number(obj, &["average_ttft", "averageTtft", "avg_ttft"]),
                average_tps: number(obj, &["average_tps", "averageTps", "avg_tps"]),
                total_requests: obj
                    .get("total_requests")
                    .or_else(|| obj.get("totalRequests"))
                    .and_then(|v| v.as_u64()),
                date: first_string(obj, &["date"]),
            })
        })
        .collect()
}

pub fn cache_aliases(aliases: Vec<RoutingAlias>) {
    *ALIAS_CACHE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Cached {
        fetched_at: Instant::now(),
        value: aliases,
    });
}

pub fn cache_stats(stats: Vec<RoutingStat>) {
    *STATS_CACHE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Cached {
        fetched_at: Instant::now(),
        value: stats,
    });
}

pub fn cached_aliases() -> Option<Vec<RoutingAlias>> {
    ALIAS_CACHE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .filter(|cache| cache.fetched_at.elapsed() < ALIAS_CACHE_TTL)
        .map(|cache| cache.value.clone())
}

pub fn cached_stats() -> Option<Vec<RoutingStat>> {
    STATS_CACHE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .filter(|cache| cache.fetched_at.elapsed() < STATS_CACHE_TTL)
        .map(|cache| cache.value.clone())
}

/// A saved `default` pool exists when the account saved at least one chute under
/// that alias. Auto then targets `default:latency` instead of composing an
/// inline pool, so the server-side routing choice the user made is honoured.
pub fn has_saved_default_pool() -> bool {
    cached_aliases().is_some_and(|aliases| {
        aliases
            .iter()
            .any(|alias| alias.alias == "default" && !alias.chute_ids.is_empty())
    })
}

/// Fetch aliases and the live stats table, refreshing both caches.
pub async fn fetch_view(client: &ChutesAccountClient) -> Result<RoutingView, String> {
    let (aliases, stats) = tokio::join!(client.model_aliases(), client.invocation_stats());
    let aliases = aliases.map_err(|error| format!("model aliases: {error}"))?;
    let stats = stats.map_err(|error| format!("model stats: {error}"))?;
    let view = RoutingView {
        aliases: parse_aliases(&aliases),
        stats: parse_stats(&stats),
    };
    cache_aliases(view.aliases.clone());
    cache_stats(view.stats.clone());
    Ok(view)
}

/// Fetch the full routing view (aliases plus stats) in the background for the
/// slash command, which cannot await inside its synchronous handler.
pub async fn prefetch_view_from_env() {
    let Ok(client) = ChutesAccountClient::from_env() else {
        return;
    };
    let _ = fetch_view(&client).await;
}

/// Fetch only the saved aliases (fast startup path used by Auto) and cache them.
pub async fn prefetch_aliases_from_env() {
    let Ok(client) = ChutesAccountClient::from_env() else {
        return;
    };
    let Ok(value) = client.model_aliases().await else {
        return;
    };
    cache_aliases(parse_aliases(&value));
}

fn first_string(obj: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        obj.get(*key)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    })
}

fn number(obj: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        obj.get(*key)
            .and_then(|value| value.as_f64())
            .filter(|value| value.is_finite())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_alias_array() {
        let value = serde_json::json!([
            {"alias": "default", "chute_ids": ["a", "b"]},
            {"alias": "fast", "chute_ids": []}
        ]);
        let aliases = parse_aliases(&value);
        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0].alias, "default");
        assert_eq!(aliases[0].chute_ids, ["a", "b"]);
    }

    #[test]
    fn parses_alias_object_shape() {
        let value = serde_json::json!({"default": ["a"], "fast": ["b", "c"]});
        let mut aliases = parse_aliases(&value);
        aliases.sort_by(|left, right| left.alias.cmp(&right.alias));
        assert_eq!(aliases[0].alias, "default");
        assert_eq!(aliases[1].chute_ids, ["b", "c"]);
    }

    #[test]
    fn parses_stats_and_keeps_missing_fields_optional() {
        let value = serde_json::json!([
            {"chute_id": "a", "name": "model-a", "average_ttft": 0.34, "average_tps": 37.1, "total_requests": 12},
            {"chute_id": "b"}
        ]);
        let stats = parse_stats(&value);
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].name, "model-a");
        assert_eq!(stats[0].average_ttft_secs, Some(0.34));
        assert_eq!(stats[1].name, "b");
        assert_eq!(stats[1].average_ttft_secs, None);
    }

    #[test]
    fn saved_default_pool_requires_a_non_empty_default_alias() {
        cache_aliases(vec![RoutingAlias {
            alias: "default".to_owned(),
            chute_ids: vec!["chute-a".to_owned()],
        }]);
        assert!(has_saved_default_pool());
        cache_aliases(vec![RoutingAlias {
            alias: "default".to_owned(),
            chute_ids: Vec::new(),
        }]);
        assert!(!has_saved_default_pool());
    }
}
