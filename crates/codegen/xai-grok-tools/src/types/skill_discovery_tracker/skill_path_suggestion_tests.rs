use std::path::PathBuf;

use crate::implementations::skills::types::SkillInfo;

use super::SkillManager;

/// Absolute in the host's own dialect.
///
/// Upstream's fixtures are POSIX literals, and on Windows a leading slash is
/// drive-*relative*: `Path::new("/repo/x").is_absolute()` is false there.
/// `suggest_skill_path` skips every registration that is not absolute, so it
/// saw an empty candidate set and seven of these tests failed here while
/// passing on upstream's Linux and macOS CI. The production guard is correct —
/// a real skill path on Windows is `C:\…` and absolute — so only the fixtures
/// needed a dialect. Keep new paths in this file going through `abs`.
fn abs(path: &str) -> String {
    #[cfg(windows)]
    {
        format!("C:{path}")
    }
    #[cfg(not(windows))]
    {
        path.to_owned()
    }
}

fn abs_path(path: &str) -> PathBuf {
    PathBuf::from(abs(path))
}

fn skill(name: &str, path: &str) -> SkillInfo {
    SkillInfo {
        name: name.to_owned(),
        path: abs(path),
        ..SkillInfo::default()
    }
}

fn seeded_manager(skills: Vec<SkillInfo>) -> SkillManager {
    let mut manager = SkillManager::new();
    manager.seed(None, None, skills, None, None, None);
    manager
}

#[test]
fn suggests_unique_registered_path_for_wrong_root() {
    let manager = seeded_manager(vec![skill(
        "code-review",
        "/home/user/.chutes-build/skills/code-review/SKILL.md",
    )]);

    let suggestion = manager
        .suggest_skill_path(&abs_path("/wrong/root/skills/code-review/SKILL.md"))
        .unwrap();

    assert_eq!(
        suggestion.display_path,
        abs_path("/home/user/.chutes-build/skills/code-review/SKILL.md")
    );
}

#[test]
fn suggests_nothing_for_ambiguous_disabled_non_skill_or_exact_requests() {
    let mut retired = skill(
        "retired",
        "/home/user/.chutes-build/skills/retired/SKILL.md",
    );
    retired.enabled = false;
    let manager = seeded_manager(vec![
        skill("review", "/repo/.chutes-build/skills/review/SKILL.md"),
        skill("review", "/home/user/.chutes-build/skills/review/SKILL.md"),
        retired,
        skill("solo", "/home/user/.chutes-build/skills/solo/SKILL.md"),
    ]);

    // Control: the manager does produce a suggestion for a clean miss.
    assert!(
        manager
            .suggest_skill_path(&abs_path("/wrong/root/solo/SKILL.md"))
            .is_some()
    );

    for requested in [
        // Two registered candidates for the name.
        "/wrong/root/review/SKILL.md",
        // Disabled skill.
        "/wrong/root/retired/SKILL.md",
        // Not a SKILL.md read.
        "/wrong/root/solo/README.md",
        // The read already targeted the registered path.
        "/home/user/.chutes-build/skills/solo/SKILL.md",
    ] {
        assert!(
            manager.suggest_skill_path(&abs_path(requested)).is_none(),
            "{requested}"
        );
    }
}

#[test]
fn requested_path_among_same_named_registrations_is_ambiguous() {
    // The failed read targets one registered path for "review"; another
    // same-named registration exists. Counting the requested path as a match
    // keeps this ambiguous (no suggestion) rather than treating the sibling
    // as a unique alternate.
    let manager = seeded_manager(vec![
        skill("review", "/repo/.chutes-build/skills/review/SKILL.md"),
        skill("review", "/home/user/.chutes-build/skills/review/SKILL.md"),
    ]);

    for requested in [
        "/repo/.chutes-build/skills/review/SKILL.md",
        "/home/user/.chutes-build/skills/review/SKILL.md",
    ] {
        assert!(
            manager.suggest_skill_path(&abs_path(requested)).is_none(),
            "{requested}"
        );
    }
}

#[test]
fn includes_model_disabled_and_held_conditional_skills() {
    let mut model_disabled = skill("manual", "/home/user/.chutes-build/skills/manual/SKILL.md");
    model_disabled.disable_model_invocation = true;
    // `paths:`-gated skills are withheld from the listing but still registered,
    // whether seeded or dynamically discovered.
    let mut gated = skill("gated", "/repo/.chutes-build/skills/gated/SKILL.md");
    gated.paths = Some(vec!["src/**".to_owned()]);
    let mut manager = seeded_manager(vec![model_disabled, gated]);

    let mut dynamic = skill(
        "conditional",
        "/home/user/.chutes-build/skills/conditional/SKILL.md",
    );
    dynamic.paths = Some(vec!["src/**".to_owned()]);
    assert!(!manager.add_discovered(vec![dynamic]));

    for requested in [
        "/wrong/root/manual/SKILL.md",
        "/wrong/root/gated/SKILL.md",
        "/wrong/root/conditional/SKILL.md",
    ] {
        assert!(
            manager.suggest_skill_path(&abs_path(requested)).is_some(),
            "{requested}"
        );
    }
}

#[test]
fn baseline_reload_updates_lookup_but_not_snapshot_names() {
    let mut manager = SkillManager::new();
    manager.set_discovery_snapshot_names(vec!["initial".to_owned()]);
    manager.seed(
        None,
        None,
        vec![skill(
            "initial",
            "/repo/.chutes-build/skills/initial/SKILL.md",
        )],
        None,
        None,
        None,
    );

    manager.update_startup_baseline(vec![skill(
        "reloaded",
        "/repo/.chutes-build/skills/reloaded/SKILL.md",
    )]);

    // Session-start names are immutable across reloads, while lookup follows
    // the current baseline: a removed skill is no longer suggested.
    assert_eq!(
        manager.discovery_snapshot_names(),
        vec!["initial".to_owned()]
    );
    assert!(
        manager
            .suggest_skill_path(&abs_path("/wrong/root/reloaded/SKILL.md"))
            .is_some()
    );
    assert!(
        manager
            .suggest_skill_path(&abs_path("/wrong/root/initial/SKILL.md"))
            .is_none()
    );
}

#[test]
fn reload_disabling_a_discovered_skill_stops_suggesting_it() {
    let path = "/repo/.chutes-build/skills/review/SKILL.md";
    let mut manager = seeded_manager(Vec::new());
    manager.add_discovered(vec![skill("review", path)]);

    let mut now_disabled = skill("review", path);
    now_disabled.enabled = false;
    manager.update_startup_baseline(vec![now_disabled]);

    // The reloaded baseline record owns the canonical path even though it is
    // disabled, so the older enabled dynamic record at the same path can
    // neither be suggested nor count as a second match.
    assert!(
        manager
            .suggest_skill_path(&abs_path("/wrong/root/review/SKILL.md"))
            .is_none()
    );
}

#[test]
fn reload_moving_a_skill_suggests_only_the_current_registration() {
    // Baseline-only move: the old file may still exist on disk, but only the
    // current registration counts, so the moved path is unique.
    let mut manager = seeded_manager(vec![skill(
        "review",
        "/repo/old/.chutes-build/skills/review/SKILL.md",
    )]);
    manager.update_startup_baseline(vec![skill(
        "review",
        "/repo/new/.chutes-build/skills/review/SKILL.md",
    )]);
    let suggestion = manager
        .suggest_skill_path(&abs_path("/wrong/root/review/SKILL.md"))
        .unwrap();
    assert_eq!(
        suggestion.display_path,
        abs_path("/repo/new/.chutes-build/skills/review/SKILL.md")
    );

    // With a stale dynamic record left at the old path, lookup cannot tell a
    // stale record from a genuinely distinct same-name skill, so it fails safe
    // with no suggestion rather than risk pointing at the wrong SKILL.md.
    let mut manager = seeded_manager(Vec::new());
    manager.add_discovered(vec![skill(
        "review",
        "/repo/old/.chutes-build/skills/review/SKILL.md",
    )]);
    manager.update_startup_baseline(vec![skill(
        "review",
        "/repo/new/.chutes-build/skills/review/SKILL.md",
    )]);
    assert!(
        manager
            .suggest_skill_path(&abs_path("/wrong/root/review/SKILL.md"))
            .is_none()
    );
}

#[test]
fn deduplicates_discovered_and_baseline_copies() {
    let path = "/repo/.chutes-build/skills/review/SKILL.md";
    let mut manager = seeded_manager(vec![skill("review", path)]);
    manager.add_discovered(vec![skill("review", path)]);

    let suggestion = manager
        .suggest_skill_path(&abs_path("/wrong/root/review/SKILL.md"))
        .unwrap();

    assert_eq!(suggestion.display_path, abs_path(path));
}

#[test]
fn rewrites_worktree_paths_to_display_cwd_but_preserves_external_paths() {
    let mut manager = seeded_manager(vec![
        skill(
            "review",
            "/real/worktree/.chutes-build/skills/review/SKILL.md",
        ),
        skill(
            "external",
            "/home/user/.chutes-build/skills/external/SKILL.md",
        ),
    ]);
    manager.real_cwd_prefix = Some(abs("/real/worktree"));
    manager.display_cwd = Some(abs("/display/project"));

    assert_eq!(
        manager
            .suggest_skill_path(&abs_path("/wrong/root/review/SKILL.md"))
            .unwrap()
            .display_path,
        abs_path("/display/project/.chutes-build/skills/review/SKILL.md")
    );
    assert_eq!(
        manager
            .suggest_skill_path(&abs_path("/wrong/root/external/SKILL.md"))
            .unwrap()
            .display_path,
        abs_path("/home/user/.chutes-build/skills/external/SKILL.md")
    );
}
