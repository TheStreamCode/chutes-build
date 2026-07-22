//! Optional Speech-to-Text transport for explicitly configured compatible endpoints.

pub mod batch;
mod streaming;
mod types;

pub use batch::transcribe_batch;
pub use streaming::{StreamingSttEvent, StreamingSttSession};
pub use types::{SttServerEvent, SttTranscriptPartial};
