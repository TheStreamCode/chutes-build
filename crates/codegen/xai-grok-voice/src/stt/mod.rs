//! xAI Speech-to-Text: streaming `wss://api.chutes.ai/v1/stt`.

pub mod batch;
mod streaming;
mod types;

pub use batch::transcribe_batch;
pub use streaming::{StreamingSttEvent, StreamingSttSession};
pub use types::{SttServerEvent, SttTranscriptPartial};
