//! Config for a process that never fetches remote settings, read through the entries another process cached.

use std::path::Path;

use chutes_config::ConfigLayers;
use chutes_config::EndpointsConfig;
use chutes_config::RemoteSettings;
use chutes_config::effective_config::CachedRemote;
use chutes_config::effective_config::CampaignEnv;
use chutes_config::effective_config::CampaignOverlay;
use chutes_config::effective_config::CampaignSources;
use chutes_config::effective_config::ConfigWithRemote;
use chutes_config::effective_config::remote_campaigns_from_settings;
use chutes_login::AuthManager;
use chutes_login::LoginConfig;

use crate::SettingsEndpoint;
use crate::settings_cache::Freshness;
use crate::settings_cache::SettingsCacheManager;
use crate::settings_cache::SettingsCacheMode;

/// Login settings and the auth file's location come from the process environment, not these inputs.
pub struct CachedConfigInputs<'a> {
    pub chutes_home: Option<&'a Path>,
    pub layers: &'a ConfigLayers,
    pub campaign_env: &'a CampaignEnv,
    pub cache_mode: SettingsCacheMode,
}

/// Uses a cached entry only if it came from the endpoint that its campaigns name.
/// A stale entry can only turn vendor hooks off.
pub fn load_config_with_cached_remote(inputs: CachedConfigInputs<'_>) -> ConfigWithRemote {
    let dismissed = inputs
        .chutes_home
        .map(chutes_config::load_dismissed_ids)
        .unwrap_or_default();
    let effective_with = |remote: Option<&RemoteSettings>| {
        CampaignOverlay::new(
            inputs.layers,
            &CampaignSources {
                remote: remote_campaigns_from_settings(remote),
                dismissed: dismissed.clone(),
                env: inputs.campaign_env.clone(),
            },
        )
        .effective
    };
    let Some(chutes_home) = inputs
        .chutes_home
        .filter(|_| chutes_config::remote_fetch_enabled_from_layers(inputs.layers))
    else {
        return ConfigWithRemote {
            effective: effective_with(None),
            remote: CachedRemote::NotApplicable,
        };
    };

    let cache_account = |remote: Option<&RemoteSettings>| {
        let effective = effective_with(remote);
        let endpoint = SettingsEndpoint::from(&EndpointsConfig::from_config_value(&effective));
        let login_config = LoginConfig::from_effective_config(&effective)
            .inspect_err(|error| {
                tracing::warn!(
                    %error,
                    "cached remote settings skipped: [login_config] unreadable"
                );
            })
            .ok()?;

        // An expired token still names its account.
        // The cache identity leaves out the token.
        let auth = AuthManager::new_with_proxy_base_url(
            chutes_home,
            login_config,
            endpoint.origin().to_owned(),
        )
        .current_or_expired()?;
        Some(endpoint.cache_account(&auth))
    };
    let entry = SettingsCacheManager::new(chutes_home, inputs.cache_mode)
        .load_for_any_client(|settings| cache_account(Some(settings)));

    let remote = match entry {
        Some((settings, Freshness::Fresh)) => CachedRemote::Entry(Box::new(settings)),
        Some((settings, Freshness::Stale)) => {
            CachedRemote::Entry(Box::new(vendor_hooks_off_only(settings)))
        }
        None if cache_account(None).is_some() => CachedRemote::Unavailable,
        None => CachedRemote::NotApplicable,
    };
    let effective = match &remote {
        CachedRemote::Entry(settings) => effective_with(Some(settings)),
        CachedRemote::NotApplicable | CachedRemote::Unavailable => effective_with(None),
    };
    ConfigWithRemote { effective, remote }
}

fn vendor_hooks_off_only(settings: RemoteSettings) -> RemoteSettings {
    RemoteSettings {
        cursor_hooks_enabled: settings.cursor_hooks_enabled.filter(|enabled| !enabled),
        claude_hooks_enabled: settings.claude_hooks_enabled.filter(|enabled| !enabled),
        ..RemoteSettings::default()
    }
}

#[cfg(test)]
#[path = "cached_config_tests.rs"]
mod tests;
