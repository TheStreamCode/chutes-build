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
const CATALOG_CACHE_TTL: Duration = Duration::from_secs(60);

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

/// A catalogue row needed to resolve pool members and stats by `chute_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingModel {
    pub id: String,
    pub chute_id: Option<String>,
    pub confidential_compute: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingView {
    pub aliases: Vec<RoutingAlias>,
    pub stats: Vec<RoutingStat>,
    pub catalog: Vec<RoutingModel>,
}

struct Cached<T> {
    fetched_at: Instant,
    value: T,
}

static ALIAS_CACHE: LazyLock<RwLock<Option<Cached<Vec<RoutingAlias>>>>> =
    LazyLock::new(|| RwLock::new(None));
static STATS_CACHE: LazyLock<RwLock<Option<Cached<Vec<RoutingStat>>>>> =
    LazyLock::new(|| RwLock::new(None));
static CATALOG_CACHE: LazyLock<RwLock<Option<Cached<Vec<RoutingModel>>>>> =
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

/// Parse the public `/v1/models` rows into the id/chute-id mapping needed by
/// alias writes and stats matching.
pub fn parse_catalog(value: &serde_json::Value) -> Vec<RoutingModel> {
    let Some(items) = value.get("data").and_then(|data| data.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let obj = item.as_object()?;
            let id = first_string(obj, &["id", "model"])?;
            Some(RoutingModel {
                id,
                chute_id: first_string(obj, &["chute_id", "chuteId"]),
                confidential_compute: obj
                    .get("confidential_compute")
                    .or_else(|| obj.get("confidentialCompute"))
                    .and_then(|value| value.as_bool()),
            })
        })
        .collect()
}

/// Resolve a model slug to its chute id. Returns the row when the slug exists
/// but the catalogue omitted `chute_id`, so callers can report the exact gap.
pub fn find_catalog_model<'a>(catalog: &'a [RoutingModel], slug: &str) -> Option<&'a RoutingModel> {
    catalog.iter().find(|model| model.id == slug)
}

/// Catalogue row for a pool member, matched by chute id.
pub fn catalog_model_for_chute<'a>(
    catalog: &'a [RoutingModel],
    chute_id: &str,
) -> Option<&'a RoutingModel> {
    catalog
        .iter()
        .find(|model| model.chute_id.as_deref() == Some(chute_id))
}

/// Display name for a pool member: the catalogue slug when known, else a short
/// form of the raw chute id.
pub fn chute_display_name(catalog: &[RoutingModel], chute_id: &str) -> String {
    catalog_model_for_chute(catalog, chute_id)
        .map(|model| model.id.clone())
        .unwrap_or_else(|| {
            if chute_id.len() > 12 {
                format!("{}…", &chute_id[..12])
            } else {
                chute_id.to_owned()
            }
        })
}

/// Append members, de-duplicating by chute id and preserving the pool cap.
pub fn add_pool_members(
    pool: &mut Vec<String>,
    ids: &[String],
    max_pool_size: usize,
) -> Result<Vec<String>, String> {
    let mut added = Vec::new();
    for id in ids {
        if pool.iter().any(|existing| existing == id) {
            continue;
        }
        if pool.len() >= max_pool_size {
            return Err(format!("pool limit is {max_pool_size} models"));
        }
        pool.push(id.clone());
        added.push(id.clone());
    }
    Ok(added)
}

/// Remove members, returning the ids that were present.
pub fn remove_pool_members(pool: &mut Vec<String>, ids: &[String]) -> Vec<String> {
    let mut removed = Vec::new();
    pool.retain(|existing| {
        if ids.iter().any(|id| id == existing) {
            removed.push(existing.clone());
            false
        } else {
            true
        }
    });
    removed
}

pub fn cache_aliases(aliases: Vec<RoutingAlias>) {
    *ALIAS_CACHE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Cached {
        fetched_at: Instant::now(),
        value: aliases,
    });
}

pub fn cache_catalog(catalog: Vec<RoutingModel>) {
    *CATALOG_CACHE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Cached {
        fetched_at: Instant::now(),
        value: catalog,
    });
}

pub fn cached_catalog() -> Option<Vec<RoutingModel>> {
    CATALOG_CACHE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .filter(|cache| cache.fetched_at.elapsed() < CATALOG_CACHE_TTL)
        .map(|cache| cache.value.clone())
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
    let (mut aliases, stats, catalog) = tokio::join!(
        client.model_aliases(),
        client.invocation_stats(),
        client.model_catalog()
    );
    let mut aliases = aliases.map_err(|error| format!("model aliases: {error}"))?;
    // The alias list can lag a just-finished write by a moment; retry briefly
    // so `routing` immediately after `routing save` shows the new pool.
    for attempt in 0..3 {
        if !parse_aliases(&aliases).is_empty() || attempt == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        aliases = client
            .model_aliases()
            .await
            .map_err(|error| format!("model aliases: {error}"))?;
    }
    let stats = stats.map_err(|error| format!("model stats: {error}"))?;
    let catalog = catalog.map_err(|error| format!("model catalog: {error}"))?;
    let view = RoutingView {
        aliases: parse_aliases(&aliases),
        stats: parse_stats(&stats),
        catalog: parse_catalog(&catalog),
    };
    cache_aliases(view.aliases.clone());
    cache_stats(view.stats.clone());
    cache_catalog(view.catalog.clone());
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
    fn parses_catalogue_chute_ids_and_resolves_names() {
        let value = serde_json::json!({"data": [
            {"id": "acme/model-a", "chute_id": "chute-a", "confidential_compute": true},
            {"id": "acme/model-b", "chute_id": "chute-b", "confidential_compute": false}
        ]});
        let catalog = parse_catalog(&value);
        assert_eq!(catalog.len(), 2);
        assert_eq!(catalog[0].chute_id.as_deref(), Some("chute-a"));
        assert_eq!(chute_display_name(&catalog, "chute-b"), "acme/model-b");
        assert_eq!(
            chute_display_name(&catalog, "unknown-id-123456"),
            "unknown-id-1…"
        );
    }

    #[test]
    fn pool_mutations_dedupe_and_respect_the_cap() {
        let mut pool = vec!["a".to_string(), "b".to_string()];
        let added =
            add_pool_members(&mut pool, &["b".to_string(), "c".to_string()], 20).expect("fits");
        assert_eq!(added, ["c"]);
        assert_eq!(pool, ["a", "b", "c"]);
        let mut full = vec!["a".to_string()];
        assert!(add_pool_members(&mut full, &["b".to_string()], 1).is_err());
        let removed = remove_pool_members(&mut pool, &["b".to_string()]);
        assert_eq!(removed, ["b"]);
        assert_eq!(pool, ["a", "c"]);
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
