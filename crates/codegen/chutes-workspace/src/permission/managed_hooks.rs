use std::path::Path;

use chutes_permission_rules::managed_policy::ManagedSettings;

/// The disabled-hooks file under `chutes_home` plus the `allow_managed_hooks_only` pin of `managed`.
pub fn disabled_hooks_snapshot(
    managed: &ManagedSettings,
    chutes_home: Option<&Path>,
) -> chutes_hooks::trust::DisabledHooks {
    chutes_hooks::trust::DisabledHooks::load(chutes_home, managed.non_managed_hooks.is_disabled())
}
