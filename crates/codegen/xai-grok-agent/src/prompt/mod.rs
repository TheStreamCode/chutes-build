//! System prompt assembly — template rendering, AGENTS.md, and skills.
pub mod agents_md;
pub mod browser_verification;
pub mod context;
pub mod ignore;
pub mod skills;
pub mod subagent_prompts;
pub mod template;
pub mod user_message;
pub mod workspace_user;

#[cfg(test)]
#[path = "prompt_contract_tests.rs"]
mod prompt_contract_tests;
