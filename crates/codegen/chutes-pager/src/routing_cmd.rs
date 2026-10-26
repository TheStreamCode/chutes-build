//! `chutes-build routing`: saved Chutes model-routing pool and live TTFT/TPS.
//!
//! Read-only by default; `add`, `remove`, and `save` are explicit commands that
//! write the account alias through the same `/model_aliases/` API the Chutes
//! dashboard uses.

use anyhow::{Context, Result};
use chutes_build_core::account::ChutesAccountClient;
use chutes_build_core::routing_pools::{
    self, RoutingAlias, RoutingModel, RoutingStat, RoutingView, add_pool_members,
    chute_display_name, find_catalog_model, remove_pool_members,
};
use clap::Subcommand;

/// A saved Chutes routing pool holds at most 20 models in the dashboard.
const MAX_POOL_SIZE: usize = 20;

#[derive(Debug, clap::Args, Clone)]
pub struct RoutingArgs {
    #[command(subcommand)]
    pub command: Option<RoutingAction>,
    /// Print the routing view or write result as JSON.
    #[arg(long, global = true)]
    pub json: bool,
}

impl RoutingAction {
    fn is_write(&self) -> bool {
        matches!(
            self,
            Self::Add { .. } | Self::Remove { .. } | Self::Save { .. }
        )
    }
}

fn writes_enabled() -> bool {
    std::env::var("CHUTES_ROUTING_EXPERIMENTAL").as_deref() == Ok("1")
}

#[derive(Debug, Subcommand, Clone)]
pub enum RoutingAction {
    /// Show the saved pool, strategies, and live model latency.
    View,
    /// Append models to a saved pool (experimental; disabled by default).
    #[command(hide = true)]
    Add {
        /// Model id or slug, as printed by `chutes-build models`.
        model: String,
        /// Alias to edit.
        #[arg(long, default_value = "default")]
        alias: String,
        /// Print the planned pool without writing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove models from a saved pool (experimental; disabled by default).
    #[command(hide = true)]
    Remove {
        /// Model id or slug.
        model: String,
        /// Alias to edit.
        #[arg(long, default_value = "default")]
        alias: String,
        /// Print the planned pool without writing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Replace a saved pool with the given models, in order (experimental; disabled by default).
    #[command(hide = true)]
    Save {
        /// Model ids or slugs, in failover order.
        #[arg(required = true)]
        models: Vec<String>,
        /// Alias to write.
        #[arg(long, default_value = "default")]
        alias: String,
        /// Print the planned pool without writing.
        #[arg(long)]
        dry_run: bool,
    },
}

pub async fn run(args: RoutingArgs) -> Result<()> {
    let client = ChutesAccountClient::from_env().context("Cannot load Chutes routing data")?;
    if args.command.as_ref().is_some_and(RoutingAction::is_write) && !writes_enabled() {
        anyhow::bail!(
            "Routing pool writes are experimental and disabled. The Chutes alias API is              eventually consistent and intermittently returns 502. Set              CHUTES_ROUTING_EXPERIMENTAL=1 to enable add/remove/save, one command at a time."
        );
    }
    match args.command {
        None | Some(RoutingAction::View) => show(&client, args.json).await,
        Some(RoutingAction::Add {
            model,
            alias,
            dry_run,
        }) => edit(&client, Edit::Add, &alias, &[model], dry_run, args.json).await,
        Some(RoutingAction::Remove {
            model,
            alias,
            dry_run,
        }) => edit(&client, Edit::Remove, &alias, &[model], dry_run, args.json).await,
        Some(RoutingAction::Save {
            models,
            alias,
            dry_run,
        }) => edit(&client, Edit::Save, &alias, &models, dry_run, args.json).await,
    }
}

async fn show(client: &ChutesAccountClient, json: bool) -> Result<()> {
    let view = routing_pools::fetch_view(client)
        .await
        .map_err(|error| anyhow::anyhow!("Chutes routing request failed: {error}"))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        print!("{}", format_human(&view));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edit {
    Add,
    Remove,
    Save,
}

async fn edit(
    client: &ChutesAccountClient,
    edit: Edit,
    alias: &str,
    models: &[String],
    dry_run: bool,
    json: bool,
) -> Result<()> {
    validate_alias(alias)?;
    let aliases = load_aliases(client, alias).await?;
    let catalog_value = client
        .model_catalog()
        .await
        .context("Cannot read the Chutes model catalog")?;
    let catalog = routing_pools::parse_catalog(&catalog_value);

    let mut ids = Vec::new();
    let mut names = Vec::new();
    for slug in models {
        let Some(model) = find_catalog_model(&catalog, slug) else {
            anyhow::bail!(
                "Unknown model '{slug}'. Run `chutes-build models` to see the current catalog."
            );
        };
        let Some(chute_id) = model.chute_id.clone() else {
            anyhow::bail!(
                "The catalog row for '{slug}' has no chute_id; it cannot be written to a routing pool."
            );
        };
        ids.push(chute_id);
        names.push(model.id.clone());
    }

    let existing = aliases
        .iter()
        .find(|entry| entry.alias == alias)
        .map(|entry| entry.chute_ids.clone())
        .unwrap_or_default();
    let mut pool = existing.clone();
    let mut deleted = false;
    match edit {
        Edit::Add => {
            add_pool_members(&mut pool, &ids, MAX_POOL_SIZE)
                .map_err(|error| anyhow::anyhow!("{error}"))?;
        }
        Edit::Remove => {
            let removed = remove_pool_members(&mut pool, &ids);
            if removed.is_empty() {
                anyhow::bail!(
                    "No requested model is in alias '{alias}': {}",
                    names.join(", ")
                );
            }
            deleted = pool.is_empty();
        }
        Edit::Save => {
            pool.clear();
            add_pool_members(&mut pool, &ids, MAX_POOL_SIZE)
                .map_err(|error| anyhow::anyhow!("{error}"))?;
        }
    }

    let result = PlannedEdit {
        alias: alias.to_owned(),
        pool: pool.clone(),
        deleted,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print!("{}", format_edit(&result, &catalog, dry_run));
    }
    if dry_run {
        return Ok(());
    }

    if existing == pool && !deleted {
        if !json {
            println!("No change: alias '{alias}' already has this pool.");
        }
        return Ok(());
    }

    if !existing.is_empty() {
        if let Err(error) = client.delete_model_alias(alias).await {
            // A missing alias is fine for the first write; any other failure is
            // real and must not be followed by a create.
            if !matches!(
                error,
                chutes_build_core::account::AccountError::Http { status: 404, .. }
            ) {
                return Err(anyhow::anyhow!("Cannot replace alias '{alias}': {error}"));
            }
        }
    }
    if deleted {
        let _ = wait_for_pool(client, alias, None).await;
        if !json {
            println!("Deleted alias '{alias}' because its pool is empty.");
        }
        return Ok(());
    }
    if let Err(error) = client.create_model_alias(alias, &pool).await {
        // Replacing deleted first; restore the previous pool rather than leave
        // the account without its alias when the create fails.
        if !existing.is_empty() {
            let _ = client.create_model_alias(alias, &existing).await;
        }
        return Err(anyhow::anyhow!("Cannot save alias '{alias}': {error}"));
    }
    if !wait_for_pool(client, alias, Some(&pool)).await && !json {
        eprintln!(
            "warning: alias '{alias}' was accepted but the list endpoint has not caught up yet"
        );
    }
    if !json {
        println!("Saved alias '{alias}' with {} model(s).", pool.len());
    }
    Ok(())
}

/// Read the alias list, retrying briefly when the target alias is not visible.
/// The endpoint can lag a write made by the previous command.
async fn load_aliases(client: &ChutesAccountClient, alias: &str) -> Result<Vec<RoutingAlias>> {
    let mut last = Vec::new();
    for attempt in 0..10 {
        let value = client
            .model_aliases()
            .await
            .context("Cannot read saved routing pools")?;
        last = routing_pools::parse_aliases(&value);
        if last.iter().any(|entry| entry.alias == alias) {
            return Ok(last);
        }
        // A non-empty list without our alias suggests a lagging read after a
        // previous write. A genuinely empty list is retried briefly, then
        // accepted so creating a brand-new alias does not stall.
        if last.is_empty() && attempt >= 4 {
            return Ok(last);
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    Ok(last)
}

/// Wait until a write is visible to the list endpoint. The API has no
/// read-after-write guarantee, so the next command must not race an update.
async fn wait_for_pool(
    client: &ChutesAccountClient,
    alias: &str,
    expected: Option<&[String]>,
) -> bool {
    for _ in 0..20 {
        let Ok(value) = client.model_aliases().await else {
            // The gateway occasionally answers 502 for a moment; keep polling.
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            continue;
        };
        let aliases = routing_pools::parse_aliases(&value);
        let current = aliases.iter().find(|entry| entry.alias == alias);
        let matches = match (current, expected) {
            (None, None) => true,
            (Some(entry), Some(expected)) => entry.chute_ids == expected,
            _ => false,
        };
        if matches {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    false
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PlannedEdit {
    alias: String,
    pool: Vec<String>,
    deleted: bool,
}

fn validate_alias(alias: &str) -> Result<()> {
    let valid = !alias.is_empty()
        && alias.len() <= 64
        && alias
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'));
    if valid {
        Ok(())
    } else {
        anyhow::bail!("Invalid alias '{alias}'; use 1-64 ASCII letters, digits, '-', '_' or '.'")
    }
}

fn format_edit(result: &PlannedEdit, catalog: &[RoutingModel], dry_run: bool) -> String {
    let mut out = if dry_run {
        format!("Planned routing pool '{}' (dry run)\n", result.alias)
    } else {
        format!("Routing pool '{}'\n", result.alias)
    };
    if result.deleted {
        out.push_str("  (empty: the alias will be deleted)\n");
        return out;
    }
    append_pool(&mut out, &result.pool, catalog);
    out
}

fn append_pool(out: &mut String, pool: &[String], catalog: &[RoutingModel]) {
    for (index, chute_id) in pool.iter().enumerate() {
        let name = chute_display_name(catalog, chute_id);
        let payg = catalog
            .iter()
            .find(|model| model.chute_id.as_deref() == Some(chute_id.as_str()))
            .and_then(|model| model.confidential_compute)
            == Some(false);
        out.push_str(&format!(
            "  {}. {}{}\n",
            index + 1,
            name,
            if payg { " [PAYG]" } else { "" }
        ));
    }
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
                "  {} ({} model{})\n",
                alias.alias,
                alias.chute_ids.len(),
                if alias.chute_ids.len() == 1 { "" } else { "s" }
            ));
            append_pool(&mut out, &alias.chute_ids, &view.catalog);
        }
    }
    out.push('\n');
    out.push_str("Strategies: default (failover), default:latency (fastest TTFT), default:throughput (highest TPS)\n");

    let stats = display_stats(&view.stats, &view.catalog);
    if !stats.is_empty() {
        out.push('\n');
        out.push_str("Model latency (live)\n");
        let total = stats.len();
        for (name, stat) in stats.into_iter().take(25) {
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
                name, ttft, tps, requests
            ));
        }
        if total > 25 {
            out.push_str(&format!("  ... {} more models\n", total - 25));
        }
    }
    out
}

/// Match stats to catalogue rows by chute id. Rows for private chutes and rows
/// that never served traffic are not part of this table.
fn display_stats<'a>(
    stats: &'a [RoutingStat],
    catalog: &'a [RoutingModel],
) -> Vec<(String, &'a RoutingStat)> {
    use std::collections::HashMap;
    let mut best: HashMap<&str, &'a RoutingStat> = HashMap::new();
    for stat in stats {
        if stat.average_ttft_secs.unwrap_or(0.0) <= 0.0 || stat.total_requests.unwrap_or(0) == 0 {
            continue;
        }
        let Some(model) = catalog
            .iter()
            .find(|model| model.chute_id.as_deref() == Some(stat.chute_id.as_str()))
        else {
            continue;
        };
        best.entry(model.id.as_str())
            .and_modify(|current| {
                if stat.total_requests.unwrap_or(0) > current.total_requests.unwrap_or(0) {
                    *current = stat;
                }
            })
            .or_insert(stat);
    }
    let mut rows: Vec<(String, &RoutingStat)> = best
        .into_iter()
        .map(|(name, stat)| (name.to_owned(), stat))
        .collect();
    rows.sort_by(|left, right| {
        left.1
            .average_ttft_secs
            .partial_cmp(&right.1.average_ttft_secs)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Vec<RoutingModel> {
        vec![
            RoutingModel {
                id: "acme/model-a".to_string(),
                chute_id: Some("a".to_string()),
                confidential_compute: Some(true),
            },
            RoutingModel {
                id: "acme/model-b".to_string(),
                chute_id: Some("b".to_string()),
                confidential_compute: Some(false),
            },
        ]
    }

    fn stat(chute_id: &str, name: &str, ttft: f64, requests: u64) -> RoutingStat {
        RoutingStat {
            chute_id: chute_id.to_string(),
            name: name.to_string(),
            average_ttft_secs: Some(ttft),
            average_tps: Some(37.1),
            total_requests: Some(requests),
            date: None,
        }
    }

    #[test]
    fn human_format_lists_pool_and_sorts_by_ttft() {
        let view = RoutingView {
            aliases: vec![RoutingAlias {
                alias: "default".to_string(),
                chute_ids: vec!["a".to_string(), "b".to_string()],
            }],
            stats: vec![
                stat("a", "acme/model-a", 0.34, 10),
                stat("b", "acme/model-b", 1.2, 5),
            ],
            catalog: catalog(),
        };
        let text = format_human(&view);
        assert!(text.contains("default (2 models)"));
        assert!(text.contains("1. acme/model-a"));
        assert!(text.contains("2. acme/model-b [PAYG]"));
        let a = text.find("acme/model-a\n").expect("a");
        let b = text.find("acme/model-b ").expect("b");
        assert!(a < b, "fastest model must come first: {text}");
    }

    #[test]
    fn human_format_handles_missing_pool() {
        let text = format_human(&RoutingView::default());
        assert!(text.contains("Saved pool: not configured"));
    }

    #[test]
    fn stats_use_catalog_names_and_skip_private_rows() {
        let stats = vec![
            stat("a", "Animij", 0.1, 99),
            stat("b", "acme/model-b", 0.2, 99),
            stat("outside", "acme/model-outside", 0.3, 99),
        ];
        let catalog = catalog();
        let rows = display_stats(&stats, &catalog);
        let names: Vec<&str> = rows.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["acme/model-a", "acme/model-b"]);
    }

    #[test]
    fn alias_validation_rejects_path_characters() {
        assert!(validate_alias("default").is_ok());
        assert!(validate_alias("team-fast_1").is_ok());
        assert!(validate_alias("bad/alias").is_err());
        assert!(validate_alias("").is_err());
    }
}
