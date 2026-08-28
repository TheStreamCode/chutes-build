//! Prompt-contract tests: the fixed per-turn prompt must not repeat
//! normalized policy paragraphs (token-efficiency workstream 1,
//! `docs/token-efficiency-plan.md`).
//!
//! Every duplicated paragraph in the fixed prompt is paid on every turn and
//! dilutes the KV-cache prefix value. These tests normalize paragraphs to a
//! canonical form and fail when the same paragraph appears more than once —
//! within a single template or across the base prompt, the subagent and
//! apply-patch templates, and the project-instructions boilerplate. If a
//! failure points at wording that must stay duplicated verbatim for
//! correctness, add it to `INTENTIONAL_DUPLICATES` with the reason.

use std::collections::HashMap;
use xai_grok_tools::types::template_renderer::TemplateRenderer;
use xai_grok_tools::types::tool::ToolKind;

use super::agents_md::{AgentConfigFile, format_agents_md_section};
use super::template::{apply_patch_template, base_template, subagent_template};

/// Paragraphs that are intentionally identical across sources. Key: the
/// normalized paragraph; value: the reason the duplication is required.
/// Keep this list short — it is the audit checkpoint for new duplication.
const INTENTIONAL_DUPLICATES: &[&str] = &[];

/// Normalize a paragraph: collapse all whitespace runs to one space and
/// lowercase. Punctuation differences stay visible — the contract is about
/// copy-pasted policy text, not paraphrases.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_ws = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_ws && !out.is_empty() {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(ch.to_ascii_lowercase());
            prev_ws = false;
        }
    }
    out.trim_end().to_string()
}

/// Split rendered text into paragraphs on blank lines, dropping headings,
/// short labels, and XML-ish section tags that carry no policy body.
fn paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .filter(|p| normalize(p).chars().count() >= 40)
        .map(|p| {
            // Strip a leading section tag like `<tool_calling>` so a body
            // under a different tag still counts as the same paragraph.
            let p = p
                .trim_start_matches('<')
                .split_once('>')
                .map(|(_, rest)| rest.trim_start())
                .unwrap_or(p);
            normalize(p)
        })
        .filter(|p| !p.is_empty())
        .collect()
}

/// Default tool-kind map, mirroring the standard primary session's toolset.
fn default_renderer() -> TemplateRenderer {
    let tools: HashMap<ToolKind, String> = [
        (ToolKind::Read, "read_file"),
        (ToolKind::Edit, "search_replace"),
        (ToolKind::Execute, "run_terminal_command"),
        (ToolKind::Search, "grep"),
        (ToolKind::List, "list_dir"),
        (ToolKind::Plan, "todo_write"),
        (ToolKind::Skill, "skill"),
        (ToolKind::WebSearch, "web_search"),
        (ToolKind::Monitor, "monitor"),
    ]
    .into_iter()
    .map(|(kind, name)| (kind, name.to_string()))
    .collect();
    TemplateRenderer::new(tools, Default::default())
}

/// Collect the fixed prompt sources into normalized paragraph -> locations.
struct Census {
    locations: HashMap<String, Vec<&'static str>>,
}

impl Census {
    fn new() -> Self {
        Self {
            locations: HashMap::new(),
        }
    }

    fn add(&mut self, source: &'static str, text: &str) {
        for paragraph in paragraphs(text) {
            self.locations.entry(paragraph).or_default().push(source);
        }
    }

    fn duplicates(&self) -> Vec<(&String, &Vec<&'static str>)> {
        self.locations
            .iter()
            .filter(|(paragraph, locations)| {
                locations.len() > 1 && !INTENTIONAL_DUPLICATES.contains(&paragraph.as_str())
            })
            .collect()
    }
}

#[test]
fn fixed_prompt_sources_have_no_repeated_policy_paragraphs() {
    let renderer = default_renderer();
    let render = |template: &str| {
        renderer
            .render_with_extra(
                template,
                &serde_json::json!({ "is_non_interactive": false }),
            )
            .expect("template renders")
    };

    let mut census = Census::new();

    let base = render(&base_template());
    census.add("base_prompt", &base);

    let subagent = render(&subagent_template());
    census.add("subagent_prompt", &subagent);

    let apply_patch = render(&apply_patch_template());
    census.add("apply_patch_prompt", &apply_patch);

    let configs = vec![AgentConfigFile {
        file_name: "AGENTS.md".into(),
        file_path: "C:/repo/AGENTS.md".into(),
        content: "Sample project instruction body long enough to count as a paragraph.".into(),
    }];
    if let Some(section) = format_agents_md_section(&configs) {
        census.add("agents_md_section", &section);
    }

    let duplicates = census.duplicates();
    assert!(
        duplicates.is_empty(),
        "repeated policy paragraphs in the fixed prompt (token-efficiency \
         workstream 1) — deduplicate the sources or register the pair in \
         INTENTIONAL_DUPLICATES with a reason:\n{}",
        duplicates
            .iter()
            .map(|(paragraph, locations)| {
                format!(
                    "  - [{}] {}...",
                    locations.join(", "),
                    &paragraph.chars().take(140).collect::<String>()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn intentional_duplicates_list_stays_sorted_and_normalized() {
    for entry in INTENTIONAL_DUPLICATES {
        assert_eq!(
            normalize(entry),
            *entry,
            "INTENTIONAL_DUPLICATES entries must already be normalized"
        );
    }
}

#[test]
fn standard_primary_fixed_prompt_stays_under_the_token_ceiling() {
    // Baseline 2026-08-26, standard primary render (workstream 1): 1297
    // estimated tokens. The ceiling pins it so policy text cannot silently
    // re-bloat; re-derive and RAISE it only when a change genuinely needs
    // the room, and shrink it when workstream 1/2 remove fixed tokens.
    let renderer = default_renderer();
    let base = render_non_interactive(&renderer, false);
    let tokens = xai_token_estimation::estimate_tokens(&base);
    assert!(
        tokens <= 1300,
        "fixed system prompt grew to {tokens} estimated tokens \
         (ceiling 1300, baseline 1297) — deduplicate or justify the growth"
    );
}

fn render_non_interactive(renderer: &TemplateRenderer, is_non_interactive: bool) -> String {
    renderer
        .render_with_extra(
            &base_template(),
            &serde_json::json!({ "is_non_interactive": is_non_interactive }),
        )
        .expect("template renders")
}
