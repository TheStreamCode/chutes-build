//! Chutes Night theme — the official Chutes dark palette.
//!
//! The canonical palette is defined in RGB (`Color::Rgb`). At startup the
//! theme is run through [`Theme::quantized`] which downgrades every color
//! to the terminal's detected capability level (256-color, 16-color, etc.).

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

/// Helper for concise const `Color::Rgb` definitions.
const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

// Chutes Night palette — graphite surfaces with a silver accent.
//
// The brand anchors are:
//   • background = #121212
//   • accent     = #a8b4c0
//
// Green is no longer the brand signal; it is kept for what it *means* —
// success and diff insertions. Blue, amber and red are retained for the same
// reason and stay softened, so the silver chrome remains the dominant voice.
#[allow(dead_code)]
mod palette {
    use super::*;

    // ── Backgrounds ─────────────────────────────────────────────────────
    pub const BG_TERMINAL: Color = rgb(15, 15, 15); // #0f0f0f — terminal well
    pub const BG_DARK: Color = rgb(16, 17, 16); // #101110 — deepest surface
    pub const BG_BASE: Color = rgb(18, 18, 18); // #121212 — official background
    pub const BG_RAISED: Color = rgb(24, 27, 25); // #181b19 — code blocks/panels
    pub const BG_HIGHLIGHT: Color = rgb(31, 36, 33); // #1f2421 — selection band
    pub const BG_HOVER: Color = rgb(39, 45, 41); // #272d29 — active hover

    // ── Text / grays ────────────────────────────────────────────────────
    pub const FG: Color = rgb(244, 246, 245); // #f4f6f5 — primary text
    pub const FG_SECONDARY: Color = rgb(190, 198, 193); // #bec6c1 — secondary text
    pub const FG_GUTTER: Color = rgb(73, 80, 76); // #49504c — dim chrome
    pub const COMMENT: Color = rgb(126, 137, 131); // #7e8983 — muted text
    pub const GRAY: Color = rgb(99, 108, 103); // #636c67 — medium gray
    pub const GRAY_BRIGHT: Color = rgb(154, 164, 158); // #9aa49e — tool text

    // ── Brand: silver ────────────────────────────────────────────────────
    //
    // The chrome a person reads as "this product" — the wordmark, the active
    // prompt border, the assistant, the model and command labels. Cool-toned on
    // purpose: the neutral grays above are warm (green-tinted, #7e8983 and
    // #9aa49e), so a cool silver separates from them instead of merging into the
    // furniture, which a warm one would.
    pub const SILVER: Color = rgb(168, 180, 192); // #a8b4c0 — brand accent
    pub const SILVER_SOFT: Color = rgb(205, 214, 223); // #cdd6df — light silver
    pub const SILVER_MUTED: Color = rgb(107, 120, 133); // #6b7885 — subdued silver

    // ── Semantic accents ─────────────────────────────────────────────────
    //
    // Green stays where green *means* something: success, and the insertion side
    // of a diff against its red counterpart. Those are not branding, and a
    // silver diff would be unreadable.
    pub const ACCENT: Color = rgb(99, 210, 151); // #63d297 — success / insertions
    pub const ACCENT_SOFT: Color = rgb(154, 229, 188); // #9ae5bc — light green
    pub const ACCENT_MUTED: Color = rgb(69, 149, 107); // #45956b — subdued green
    pub const BLUE: Color = rgb(124, 167, 217); // #7ca7d9 — informational
    pub const CYAN: Color = rgb(117, 198, 190); // #75c6be — verification/running
    pub const AMBER: Color = rgb(224, 183, 108); // #e0b76c — plan/warning
    pub const ORANGE: Color = rgb(218, 151, 101); // #da9765 — paths/numbers
    pub const RED: Color = rgb(233, 124, 136); // #e97c88 — errors/deletions

    pub const RED_DARK: Color = rgb(62, 24, 30); // #3e181e — deletion background
    pub const ACCENT_DARK: Color = rgb(21, 58, 41); // #153a29 — insertion background
}
use palette::*;

impl Theme {
    /// Chutes Night theme — graphite surfaces with the silver Chutes accent.
    ///
    /// Colors are defined in RGB. Call [`Theme::quantized`] to downgrade
    /// them to the terminal's supported color level before rendering.
    pub const fn chutesnight() -> Self {
        Self {
            bg_base: BG_BASE,
            bg_light: BG_HIGHLIGHT,
            bg_dark: BG_RAISED,
            bg_highlight: BG_HIGHLIGHT,
            bg_hover: BG_HOVER,
            bg_terminal: BG_TERMINAL,

            accent_user: FG_SECONDARY,
            accent_assistant: SILVER,
            accent_thinking: SILVER_SOFT,
            accent_tool: GRAY_BRIGHT,
            accent_system: BLUE,
            accent_error: RED,
            accent_success: ACCENT,
            accent_running: SILVER,
            accent_skill: SILVER_SOFT,

            text_primary: FG,
            text_secondary: FG_SECONDARY,

            gray_dim: FG_GUTTER,
            gray: COMMENT,
            gray_bright: GRAY_BRIGHT,

            command: SILVER_SOFT,
            path: ORANGE,
            running: CYAN,
            warning: AMBER,

            fuzzy_accent: SILVER,

            accent_plan: AMBER,

            accent_verify: CYAN,

            accent_remember: SILVER_MUTED,

            selection_border: rgb(88, 99, 110),
            prompt_border: rgb(48, 54, 50),
            prompt_border_active: SILVER_MUTED,
            hover_border: rgb(42, 48, 44),

            accent_model: SILVER,

            scrollbar_bg: BG_DARK,
            scrollbar_fg: BG_HIGHLIGHT,

            diff_delete_bg: RED_DARK,
            diff_delete_fg: RED,
            diff_insert_bg: ACCENT_DARK,
            diff_insert_fg: ACCENT,
            diff_equal_fg: COMMENT,
            diff_gutter_fg: COMMENT,

            bg_visual: rgb(42, 47, 53),

            paste_bg: BG_DARK,
            paste_fg: FG_SECONDARY,
            paste_dim: FG_GUTTER,

            md_heading_h1: SILVER,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: BLUE,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: SILVER_SOFT,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: GRAY_BRIGHT,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: COMMENT,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: GRAY,
            md_heading_h6_mod: Modifier::empty(),
            md_code: CYAN,
            md_task_checked: ACCENT,
            md_task_unchecked: FG_SECONDARY,
            md_muted: COMMENT,
            md_code_bg: BG_RAISED,
            md_text: FG_SECONDARY,
            link_fg: BLUE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chutesnight_theme() {
        let theme = Theme::chutesnight();
        assert_eq!(theme.bg_base, Color::Rgb(18, 18, 18));
        assert_eq!(theme.bg_terminal, Color::Rgb(15, 15, 15));
        // Brand chrome is silver; green survives only where it carries meaning.
        assert_eq!(theme.accent_assistant, Color::Rgb(168, 180, 192));
        assert_eq!(theme.accent_model, Color::Rgb(168, 180, 192));
        assert_eq!(theme.accent_success, Color::Rgb(99, 210, 151));
        assert_eq!(theme.diff_insert_fg, Color::Rgb(99, 210, 151));
        assert_eq!(theme.text_primary, Color::Rgb(244, 246, 245));
    }
}
