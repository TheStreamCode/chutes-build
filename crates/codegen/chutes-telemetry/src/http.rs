//! Origin/client identification used by the telemetry engine.
//!
//! [`OriginClientInfo`] is owned by `chutes-sampler` (so `SamplerConfig` can use it without depending on shell).
//! Re-exported here so the telemetry engine can label events without depending on shell or sampler internals beyond the type itself.

pub use chutes_sampler::OriginClientInfo;

/// Construct an [`OriginClientInfo`] from the `CHUTES_BUILD_CLIENT_NAME` / `CHUTES_BUILD_CLIENT_VERSION` env vars.
/// Returns `None` when `CHUTES_BUILD_CLIENT_NAME` is unset.
/// This is a free function rather than an inherent method because the type lives in another crate.
pub fn origin_client_info_from_env() -> Option<OriginClientInfo> {
    std::env::var("CHUTES_BUILD_CLIENT_NAME")
        .ok()
        .map(|product| OriginClientInfo {
            product,
            version: std::env::var("CHUTES_BUILD_CLIENT_VERSION").ok(),
        })
}
