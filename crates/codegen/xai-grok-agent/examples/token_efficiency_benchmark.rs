//! Deterministic, local-only prompt efficiency benchmark.
//!
//! Run with:
//! `cargo run -p xai-grok-agent --example token_efficiency_benchmark`

use serde::Serialize;
use xai_grok_agent::prompt::agents_md::{
    AgentConfigFile, LEGACY_AGENTS_MD_REMINDER_PREFIX, format_agents_md_section,
};
use xai_grok_tools::types::output::{MediaArtifact, MediaArtifactKind};

#[derive(Serialize)]
struct Measurement {
    scenario: &'static str,
    baseline_chars: usize,
    candidate_chars: usize,
    baseline_estimated_tokens: u64,
    candidate_estimated_tokens: u64,
    estimated_tokens_saved: u64,
    reduction_percent: f64,
    invariant_passed: bool,
}

fn measurement(
    scenario: &'static str,
    baseline: &str,
    candidate: &str,
    invariant_passed: bool,
) -> Measurement {
    let baseline_tokens = xai_token_estimation::estimate_tokens(baseline);
    let candidate_tokens = xai_token_estimation::estimate_tokens(candidate);
    let saved = baseline_tokens.saturating_sub(candidate_tokens);
    let reduction_percent = if baseline_tokens == 0 {
        0.0
    } else {
        saved as f64 * 100.0 / baseline_tokens as f64
    };
    Measurement {
        scenario,
        baseline_chars: baseline.len(),
        candidate_chars: candidate.len(),
        baseline_estimated_tokens: baseline_tokens,
        candidate_estimated_tokens: candidate_tokens,
        estimated_tokens_saved: saved,
        reduction_percent,
        invariant_passed,
    }
}

fn legacy_agents_render(configs: &[AgentConfigFile]) -> String {
    let mut output = String::from(LEGACY_AGENTS_MD_REMINDER_PREFIX);
    output.push_str(
        " (ordered from repo root to current directory - deeper files take precedence on conflicts):\n",
    );
    for config in configs {
        output.push_str(&format!("\n## From: {}\n", config.file_path));
        output.push_str(&config.content);
        output.push('\n');
    }
    output.push_str("\nFollow these instructions exactly. When working in subdirectories not listed above, check for additional project instruction files (AGENTS.md, Claude.md, etc.).");
    output.push_str("\n</system-reminder>");
    output
}

fn main() {
    let shared_instructions =
        "Use PowerShell on Windows.\nRun focused tests before reporting success.";
    let configs = vec![
        AgentConfigFile {
            file_name: "AGENTS.md".into(),
            file_path: "C:/repo/AGENTS.md".into(),
            content: shared_instructions.into(),
        },
        AgentConfigFile {
            file_name: "rules.md".into(),
            file_path: "C:/repo/.chutes-build/rules/rules.md".into(),
            content: "---\ndescription: shared rules\n---\nUse narrow, reversible edits.".into(),
        },
        AgentConfigFile {
            file_name: "AGENTS.md".into(),
            file_path: "C:/repo/crates/app/AGENTS.md".into(),
            content: shared_instructions.into(),
        },
    ];
    let legacy_agents = legacy_agents_render(&configs);
    let optimized_agents = format_agents_md_section(&configs).expect("fixture is non-empty");
    let agent_invariant = optimized_agents.contains(shared_instructions)
        && optimized_agents.contains("Use narrow, reversible edits.")
        && optimized_agents.contains("C:/repo/crates/app/AGENTS.md")
        && !optimized_agents.contains("## From: C:/repo/AGENTS.md");

    let artifact = MediaArtifact {
        schema_version: MediaArtifact::SCHEMA_VERSION,
        kind: MediaArtifactKind::Video,
        path: "C:/repo/.chutes-build/media/demo.mp4".into(),
        mime_type: "video/mp4".into(),
        byte_len: 18_420_736,
        provenance_path: Some("C:/repo/.chutes-build/media/demo.json".into()),
        provider: "chutes".into(),
        model: "example-video-model".into(),
        cost: Some(0.42),
    };
    let legacy_media = serde_json::to_string_pretty(&artifact).expect("serializable fixture");
    let compact_media = artifact.prompt_text();
    let media_invariant = compact_media.contains("demo.mp4")
        && compact_media.contains("video")
        && !compact_media.contains("example-video-model")
        && !compact_media.contains("0.42");

    let measurements = vec![
        measurement(
            "project_instruction_exact_body_dedup",
            &legacy_agents,
            &optimized_agents,
            agent_invariant,
        ),
        measurement(
            "typed_media_compact_model_receipt",
            &legacy_media,
            &compact_media,
            media_invariant,
        ),
    ];
    println!(
        "{}",
        serde_json::to_string_pretty(&measurements).expect("benchmark report serializes")
    );
}
