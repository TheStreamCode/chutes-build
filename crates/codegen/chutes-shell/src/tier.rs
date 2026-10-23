//! Subscription-tier classification shared across the shell and the pager.
//!
//! The subscription tier reaches the client as a free-form **display-name string**.
//! It comes from the account `/settings` `subscription_tier_display`, or from the numeric JWT `tier` claim.
//! [`crate::agent::mvp_agent::jwt_tier_claim`] maps the numeric claim (`0` is free, paid levels pass through as their number).
//! There is no shared enum, so gating decisions classify the string here in ONE place.
//! That keeps the pager's cosmetic slash-command gate and the shell's capability (toolset) gate from drifting apart.
//!
//! "Restricted" tiers are the free tier and an absent account plan, the tiers the server
//! zero-limits on the Imagine and voice endpoints. Any other plan name is unrestricted (**fail-open**).

/// Whether a **known** subscription-tier display name is a gated tier: no paid plan or an explicit "Free".
/// Chutes has no free subscription tier (pricing is pay-as-you-go plus the Plus/Pro/Enterprise plans), so the
/// practical case is an absent account plan; every recognised Chutes plan is unrestricted (fail-open).
/// The pager treats absence as restricted (cosmetic, recovers live on the next settings update).
/// The shell treats absence as unrestricted (fail-open): the server is the authority on per-tier limits, so never withhold a capability on a guess.
pub fn is_restricted_tier_name(tier: &str) -> bool {
    let t = tier.trim().to_ascii_lowercase();
    t.is_empty() || t == "free"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restricted_names() {
        assert!(is_restricted_tier_name(""));
        assert!(is_restricted_tier_name("   "));
        assert!(is_restricted_tier_name("Free"));
        assert!(is_restricted_tier_name("free"));
    }

    #[test]
    fn unrestricted_names() {
        assert!(!is_restricted_tier_name("Pro"));
        assert!(!is_restricted_tier_name("Team"));
        assert!(!is_restricted_tier_name("Enterprise"));
        assert!(!is_restricted_tier_name("Business"));
        assert!(!is_restricted_tier_name("team_plan"));
        // API keys are not free-tier gated.
        assert!(!is_restricted_tier_name("api_key"));
        assert!(!is_restricted_tier_name("API Key"));
        // Unknown future tiers fail open.
        assert!(!is_restricted_tier_name("some_new_plan"));
    }
}
