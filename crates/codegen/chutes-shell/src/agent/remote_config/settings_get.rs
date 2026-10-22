//! Each function here calls the `chutes-cloud-config` startup settings getter with the shell's `policy_repair_pending` check.

use std::time::Duration;

use chutes_login::{ChutesAuth, LoginConfig};
use tokio_util::sync::CancellationToken;

use crate::util::config::RemoteSettings;
use chutes_cloud_config::managed_config::policy_repair_pending;

#[cfg(any(test, feature = "test-support"))]
pub use chutes_cloud_config::settings_get::reset_startup_settings_for_tests;

pub use chutes_cloud_config::settings_get::{SettingsOutcome, SettingsQuery, SettingsWait};

pub fn is_eligible(query: &SettingsQuery) -> bool {
    chutes_cloud_config::settings_get::is_eligible(query, policy_repair_pending)
}

#[cfg(any(test, feature = "test-support"))]
pub async fn get_settings(query: SettingsQuery) -> SettingsOutcome {
    chutes_cloud_config::settings_get::get_settings(query, policy_repair_pending).await
}

pub async fn await_startup_settings(
    query: SettingsQuery,
    deadline: Duration,
    cancel: &CancellationToken,
) -> SettingsWait {
    chutes_cloud_config::settings_get::await_startup_settings(
        query,
        deadline,
        cancel,
        policy_repair_pending,
    )
    .await
}

pub fn consume_wait(
    wait: SettingsWait,
    auth: Option<&ChutesAuth>,
    login_config: &LoginConfig,
) -> Option<RemoteSettings> {
    chutes_cloud_config::settings_get::consume_wait(wait, auth, login_config, policy_repair_pending)
}

#[cfg(any(test, feature = "test-support"))]
pub async fn get_startup_settings(query: SettingsQuery) -> SettingsOutcome {
    chutes_cloud_config::settings_get::get_startup_settings(query, policy_repair_pending).await
}

pub fn warm_startup_settings(query: SettingsQuery) {
    chutes_cloud_config::settings_get::warm_startup_settings(query, policy_repair_pending);
}

pub fn block_on_startup_settings(
    query: SettingsQuery,
    deadline: Duration,
    cancel: &CancellationToken,
) -> SettingsWait {
    chutes_cloud_config::settings_get::block_on_startup_settings(
        query,
        deadline,
        cancel,
        policy_repair_pending,
    )
}
