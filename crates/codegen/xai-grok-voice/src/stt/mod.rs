//! Optional Speech-to-Text transport for explicitly configured compatible endpoints.

mod streaming;
mod types;

pub use streaming::{StreamingSttEvent, StreamingSttSession};
pub use types::{SttServerEvent, SttTranscriptPartial};
