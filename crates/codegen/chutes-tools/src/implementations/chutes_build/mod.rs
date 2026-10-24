//! New-architecture tool implementations (NewTool trait).
//!
//! Each sub-module here contains a tool that implements `NewTool` instead
//! of the old `Tool` trait. During migration, old implementations live in
//! `implementations/<tool>/` and new implementations live in
//! `implementations/chutes_build/<tool>/`.
//!
//! The [`register_all()`] function is the single entry-point for wiring up
//! the standard toolset. It inserts shared resources (`Terminal`,
//! `AvailableSkills`, `BashParams`) and registers every built-in tool.
#[path = "app_builder_stub.rs"]
pub mod app_builder;
pub mod ask_user_question;
pub mod bash;
#[path = "deploy_app_stub.rs"]
pub mod deploy_app;
pub mod enter_plan_mode;
pub mod exit_plan_mode;
pub mod grep;
#[path = "init_or_update_app_stub.rs"]
pub mod init_or_update_app;
pub mod kill_task;
pub mod list_dir;
pub mod lsp;
pub mod monitor;
pub mod read_file;
pub mod scheduler;
pub mod search_replace;
pub mod send_feedback;
pub mod send_subagent_message;
pub(crate) mod storage;
pub mod task;
pub mod task_output;
pub mod todo;
pub mod update_goal;
pub mod web_fetch;
pub mod web_search;
pub mod workflow;
pub use app_builder::AppBuilderDeployerConfig;
pub use ask_user_question::AskUserQuestionTool;
pub use bash::BashTool;
pub use deploy_app::DEPLOY_APP_TOOL_NAME;
pub use enter_plan_mode::EnterPlanModeTool;
pub use exit_plan_mode::ExitPlanModeTool;
pub use grep::GrepTool;
pub use init_or_update_app::INIT_OR_UPDATE_APP_TOOL_NAME;
pub use kill_task::{KillTaskTool, KillTerminalCommandTool};
pub use list_dir::ListDirTool;
pub use lsp::LspTool;
pub use monitor::tool::MonitorTool;
pub use read_file::ReadFileTool;
pub use scheduler::create::{
    SCHEDULER_CREATE_TOOL_NAME, SchedulerCreateTool, loop_schedule_instruction, loop_usage_message,
};
pub use scheduler::delete::{SCHEDULER_DELETE_TOOL_NAME, SchedulerDeleteTool};
pub use scheduler::list::SchedulerListTool;
pub use search_replace::SearchReplaceTool;
pub use send_feedback::{SEND_FEEDBACK_TOOL_NAME, SendFeedbackTool};
pub use send_subagent_message::{
    SEND_SUBAGENT_MESSAGE_TOOL_NAME, SendSubagentMessageDisposition, SendSubagentMessageTool,
};
pub use task::{TaskTool, is_task_tool_id};
pub use task_output::{GetTerminalCommandOutputTool, TaskOutputTool, WaitTasksTool};
pub use todo::TodoWriteTool;
pub use update_goal::{UPDATE_GOAL_TOOL_NAME, UpdateGoalTool};
// `/imagine` and `/imagine-video` stay: they expand into the Chutes-native
// `generate_media` tool. The legacy Imagine tool implementations that used to
// re-export these constants were removed in 2.0.3.
pub use chutes_tools_api::slash_commands::{
    IMAGINE_COMMAND_NAME, IMAGINE_VIDEO_COMMAND_NAME, imagine_instruction, imagine_usage_message,
    imagine_video_instruction, imagine_video_usage_message,
};
pub use web_fetch::{WebFetchClient, WebFetchConfig, WebFetchParams, WebFetchTool};
pub use web_search::WebSearchTool;
pub use workflow::{
    WORKFLOW_TOOL_NAME, WorkflowTool, is_workflow_tool, is_workflow_tool_id,
    workflow_tool_short_name,
};
