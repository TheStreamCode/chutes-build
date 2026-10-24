//! Embedded bundled skills materialized under `<Chutes Build home>/bundled/skills`.
//!
//! Skills discovery reads `bundled/skills/<name>/SKILL.md` as the lowest-priority
//! skill scope, so a user or repository skill with the same name shadows it. The
//! files are written at startup from `include_str!` assets shipped in the binary.

use std::path::Path;

/// Bundled skills shipped with the binary: directory name and `SKILL.md` body.
const BUNDLED_SKILLS: &[(&str, &str)] = &[(
    "chutes-platform",
    include_str!("../assets/bundled-skills/chutes-platform/SKILL.md"),
)];

/// Write the embedded skills into `<chutes_home>/bundled/skills/`.
///
/// Rewrites a file only when its content differs, so an unchanged skill keeps its
/// mtime and a user edit is detected by the normal skill-discovery path. Never
/// deletes anything: the remote subagent bundle shares this directory tree.
pub fn extract_bundled_skills(chutes_home: &Path) {
    for (name, body) in BUNDLED_SKILLS {
        let dir = chutes_home.join("bundled").join("skills").join(name);
        let path = dir.join("SKILL.md");
        if std::fs::read_to_string(&path).is_ok_and(|existing| existing == *body) {
            continue;
        }
        if let Err(error) = std::fs::create_dir_all(&dir) {
            tracing::warn!(%error, skill = name, "Failed to create bundled skill directory");
            continue;
        }
        if let Err(error) = std::fs::write(&path, body) {
            tracing::debug!(%error, skill = name, "Failed to extract bundled skill");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_skill_bodies_have_frontmatter_and_content() {
        for (name, body) in BUNDLED_SKILLS {
            assert!(
                body.starts_with("---\nname: "),
                "bundled skill `{name}` must start with YAML frontmatter"
            );
            assert!(
                body.contains(&format!("name: {name}")),
                "bundled skill `{name}` frontmatter name must match its directory"
            );
            assert!(
                body.contains("description:"),
                "bundled skill `{name}` must describe when to use it"
            );
            assert!(
                body.trim().len() > 1_000,
                "bundled skill `{name}` is suspiciously short"
            );
        }
    }

    #[test]
    fn extract_writes_the_skill_and_keeps_unchanged_files() {
        let home = tempfile::tempdir().expect("tempdir");
        extract_bundled_skills(home.path());
        let path = home
            .path()
            .join("bundled")
            .join("skills")
            .join("chutes-platform")
            .join("SKILL.md");
        let written = std::fs::read_to_string(&path).expect("skill extracted");
        assert!(written.contains("name: chutes-platform"));

        let first_modified = std::fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .expect("mtime");
        std::thread::sleep(std::time::Duration::from_millis(10));
        extract_bundled_skills(home.path());
        let second_modified = std::fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .expect("mtime");
        assert_eq!(
            first_modified, second_modified,
            "unchanged skill was rewritten"
        );
    }

    #[test]
    fn extract_repairs_a_tampered_skill() {
        let home = tempfile::tempdir().expect("tempdir");
        let dir = home
            .path()
            .join("bundled")
            .join("skills")
            .join("chutes-platform");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("SKILL.md"), "tampered").expect("write");
        extract_bundled_skills(home.path());
        let written = std::fs::read_to_string(dir.join("SKILL.md")).expect("skill");
        assert!(written.contains("name: chutes-platform"));
        assert!(!written.contains("tampered"));
    }
}
