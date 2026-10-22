/// Apply auth headers to outbound visibility requests.
/// Implemented by `chutes-shell::util::chutes_auth_credentials::ChutesAuthCredentials`.
/// Shell owns credential construction; data-collector builds the request without importing shell types.
pub trait HttpAuth: Send + Sync {
    fn apply(&self, builder: reqwest::RequestBuilder, base_url: &str) -> reqwest::RequestBuilder;
}
