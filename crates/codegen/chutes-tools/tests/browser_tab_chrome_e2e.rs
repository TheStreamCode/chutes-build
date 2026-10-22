//! End-to-end tests for the `browser_tab` and `browser_network_details` tools
//! against a real Chrome.
//!
//! These drive the tools themselves against a real Chrome, so the output
//! the model actually sees is what gets asserted.
//!
//! Ignored by default — CI has no Chrome. Run them with:
//!
//! ```bash
//! cargo test -p chutes-tools --features browser --test browser_tab_chrome_e2e \
//!     -- --ignored --test-threads=1
//! ```
