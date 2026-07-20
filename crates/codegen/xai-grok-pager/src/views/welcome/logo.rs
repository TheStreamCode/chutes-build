//! Chutes Build wordmark with an entrance reveal and continuous shimmer.
//!
//! The brand wordmark is intentionally invariant across terminals. Chutes
//! Build already configures UTF-8 output on Windows, and substituting an ASCII
//! fallback would change the approved logo rather than merely adapt UI chrome.

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::render::color::blend_color;
use crate::theme::Theme;

const LOGO: &str = include_str!("../../../assets/logo/logo07.txt");
const LOGO_SMALL: &str = include_str!("../../../assets/logo/logo05.txt");

/// Height at or above which the small logo is shown (below it, no logo).
const SMALL_LOGO_MIN_HEIGHT: u16 = 22;
/// Height at or above which the full two-line wordmark is shown.
const FULL_LOGO_MIN_HEIGHT: u16 = 30;

fn pick_logo(window_height: u16) -> Option<&'static str> {
    pick_logo_for(window_height)
}

/// Pure tier selection for the approved full and compact wordmarks.
fn pick_logo_for(window_height: u16) -> Option<&'static str> {
    if window_height < SMALL_LOGO_MIN_HEIGHT {
        None
    } else if window_height < FULL_LOGO_MIN_HEIGHT {
        Some(LOGO_SMALL)
    } else {
        Some(LOGO)
    }
}

fn non_empty_lines(logo: &str) -> impl Iterator<Item = &str> {
    // `str::lines` already omits the terminal newline. Preserve intentional
    // interior blank rows between CHUTES and BUILD.
    logo.lines()
}

fn count_lines(logo: &str) -> u16 {
    non_empty_lines(logo).count() as u16
}

fn visual_width(logo: &str) -> u16 {
    non_empty_lines(logo)
        .map(unicode_width::UnicodeWidthStr::width)
        .max()
        .unwrap_or(24) as u16
}

/// Animation phase in seconds since the first render. Wall-clock based so the
/// shimmer speed is independent of the frame rate.
fn anim_phase_secs() -> f32 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// Shimmer redraw cadence in frames per second. The sweep is slow, so a few fps
/// looks smooth while sparing the long-lived welcome screen from full-rate
/// repaints.
const SHIMMER_FPS: f32 = 12.0;

/// Duration of the one-shot startup reveal. Kept below a second so the brand
/// moment reads clearly without delaying interaction.
const ENTRANCE_SECS: f32 = 0.9;

/// Quantized shimmer frame for the current wall-clock phase. The welcome screen
/// redraws only when this advances, throttling the animation to ~`SHIMMER_FPS`
/// rather than the full event-loop tick rate.
pub fn shimmer_frame() -> u64 {
    (anim_phase_secs() * SHIMMER_FPS) as u64
}

/// Reveal progress in `[0, 1]`, eased so the logo arrives decisively and then
/// settles. This is intentionally independent of terminal color support.
fn entrance_progress(secs: f32) -> f32 {
    let t = (secs / ENTRANCE_SECS).clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Per-glyph shine opacity in `[0, 1]` at normalized diagonal position `diag`
/// (0 = bottom-left .. 1 = top-right) and animation time `secs`. A raised-cosine
/// band continuously sweeps bottom-left → top-right while a gentle global
/// pulse breathes underneath it. 0 keeps the resting green, 1 is bright white.
fn shine_opacity(diag: f32, secs: f32) -> f32 {
    const BAND: f32 = 0.24; // half-width of the shine band
    const CYCLE: f32 = 3.2; // seconds per continuous sweep
    const SHINE: f32 = 0.78; // peak green-to-white strength
    const PULSE: f32 = 0.08; // global breathing amount
    const PULSE_SECS: f32 = 4.8; // breathing period

    let p = (secs % CYCLE) / CYCLE;
    let band_pos = -BAND + p * (1.0 + 2.0 * BAND);
    let pulse = PULSE * (0.5 - 0.5 * (std::f32::consts::TAU * secs / PULSE_SECS).cos());

    let d = (diag - band_pos).abs();
    let shine = if d < BAND {
        0.5 * (1.0 + (std::f32::consts::PI * d / BAND).cos())
    } else {
        0.0
    };
    (pulse + SHINE * shine).clamp(0.0, 1.0)
}

/// True-color and 256-color terminals get a smooth gradient. Named ANSI
/// colors cannot be interpolated safely, so they receive a crisp two-color
/// glint instead of silently collapsing to the resting color.
fn shine_color(base: Color, hilite: Color, opacity: f32) -> Color {
    blend_color(base, hilite, opacity).unwrap_or_else(
        || {
            if opacity >= 0.22 { hilite } else { base }
        },
    )
}

fn render_into(area: Rect, buf: &mut Buffer, theme: &Theme, logo: &str) {
    let lines: Vec<&str> = non_empty_lines(logo).collect();
    let rows = lines.len().max(1) as f32;
    let cols = lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    let secs = anim_phase_secs();
    let reveal = entrance_progress(secs);

    // Blend each glyph from the Chutes green toward primary text by its shine
    // opacity, so the sheen stays visible even when terminal color quantization
    // collapses the two green accent shades. Adjacent glyphs
    // that land on the same blended color share one Span to hold down the
    // per-frame allocation.
    let base = theme.accent_assistant;
    let hilite = theme.text_primary;
    let logo_lines: Vec<Line> = lines
        .iter()
        .enumerate()
        .map(|(row, line)| {
            let mut spans: Vec<Span> = Vec::new();
            let mut run = String::new();
            let mut run_color: Option<Color> = None;
            for (col, ch) in line.chars().enumerate() {
                // Sweep along the bottom-left → top-right diagonal: the
                // coordinate grows as col increases and row decreases.
                let diag = (col as f32 + (rows - 1.0 - row as f32)) / (cols + rows);
                // Reveal left-to-right with a small row stagger. Whitespace is
                // retained so centered alignment never jumps between frames.
                let reveal_pos = (col as f32 + row as f32 * 0.42) / (cols + rows * 0.42);
                let visible = ch.is_whitespace() || reveal + 0.08 >= reveal_pos;
                let glyph = if visible { ch } else { ' ' };
                let color = shine_color(base, hilite, shine_opacity(diag, secs));
                if run_color != Some(color) {
                    if let Some(prev) = run_color {
                        spans.push(Span::styled(
                            std::mem::take(&mut run),
                            Style::default().fg(prev),
                        ));
                    }
                    run_color = Some(color);
                }
                run.push(glyph);
            }
            if let Some(prev) = run_color {
                spans.push(Span::styled(run, Style::default().fg(prev)));
            }
            Line::from(spans).alignment(Alignment::Center)
        })
        .collect();
    Paragraph::new(logo_lines).render(area, buf);
}

pub fn logo_line_count(window_height: u16) -> u16 {
    pick_logo(window_height).map_or(0, count_lines)
}

pub fn logo_visual_width(window_height: u16) -> u16 {
    pick_logo(window_height).map_or(24, visual_width)
}

pub fn render_logo(area: Rect, buf: &mut Buffer, theme: &Theme, window_height: u16) {
    if let Some(logo) = pick_logo(window_height) {
        render_into(area, buf, theme, logo);
    }
}

/// The hero box always shows the full logo: it is laid out beside the menu, so
/// it fits whenever the box does. These report and render that logo directly,
/// independent of the height-based [`pick_logo`] tiers used by the stacked
/// layout.
pub fn full_logo_line_count() -> u16 {
    count_lines(LOGO)
}

pub fn full_logo_visual_width() -> u16 {
    visual_width(LOGO)
}

pub fn render_full_logo(area: Rect, buf: &mut Buffer, theme: &Theme) {
    render_into(area, buf, theme, LOGO);
}

/// Line count of the small logo used in minimal's committed welcome card
/// across all supported terminals.
pub fn compact_logo_line_count() -> u16 {
    count_lines(LOGO_SMALL)
}

/// Render the small centered logo into `area` for minimal's welcome card.
pub fn render_compact_logo(area: Rect, buf: &mut Buffer, theme: &Theme) {
    render_into(area, buf, theme, LOGO_SMALL);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_sizes_by_height() {
        assert!(pick_logo_for(SMALL_LOGO_MIN_HEIGHT - 1).is_none());
        assert_eq!(pick_logo_for(SMALL_LOGO_MIN_HEIGHT), Some(LOGO_SMALL));
        assert_eq!(pick_logo_for(FULL_LOGO_MIN_HEIGHT - 1), Some(LOGO_SMALL));
        assert_eq!(pick_logo_for(FULL_LOGO_MIN_HEIGHT), Some(LOGO));
    }

    #[test]
    fn hero_box_always_uses_full_logo() {
        // The box renders the full logo regardless of height (it's laid out
        // beside the menu), and it's the large variant — never the small one.
        assert_eq!(full_logo_line_count(), count_lines(LOGO));
        assert_eq!(full_logo_visual_width(), visual_width(LOGO));
        assert!(full_logo_line_count() > count_lines(LOGO_SMALL));
        assert!(full_logo_visual_width() > visual_width(LOGO_SMALL));
    }

    #[test]
    fn compact_logo_line_count_matches_small_logo_when_visible() {
        // The minimal welcome card budgets exactly the small logo's rows. When
        // visible, every terminal uses the same approved compact wordmark.
        assert_eq!(compact_logo_line_count(), count_lines(LOGO_SMALL));
        assert!(compact_logo_line_count() > 0);
    }

    #[test]
    fn shine_opacity_stays_in_unit_range() {
        let mut secs = 0.0;
        while secs < 10.0 {
            for i in 0..=20 {
                let diag = i as f32 / 20.0;
                let op = shine_opacity(diag, secs);
                assert!(
                    (0.0..=1.0).contains(&op),
                    "opacity {op} out of range at diag {diag}, secs {secs}"
                );
            }
            secs += 0.13;
        }
    }

    #[test]
    fn named_ansi_colors_keep_a_visible_shine() {
        assert_eq!(shine_color(Color::Green, Color::White, 0.1), Color::Green);
        assert_eq!(shine_color(Color::Green, Color::White, 0.8), Color::White);
    }

    #[test]
    fn shine_band_sweeps_across() {
        // The brightest point along the diagonal advances left → right as the
        // sweep progresses through its active phase.
        let brightest = |secs: f32| -> f32 {
            (0..=100)
                .map(|i| i as f32 / 100.0)
                .max_by(|a, b| {
                    shine_opacity(*a, secs)
                        .partial_cmp(&shine_opacity(*b, secs))
                        .unwrap()
                })
                .unwrap()
        };
        let early = brightest(0.4);
        let mid = brightest(1.2);
        let late = brightest(2.0);
        assert!(early < mid, "early {early} should precede mid {mid}");
        assert!(mid < late, "mid {mid} should precede late {late}");
    }

    #[test]
    fn shine_wraps_cleanly_between_sweeps() {
        // At the cycle boundary the band is off the logo and only the gentle
        // global pulse remains, preventing a full-frame flash on wrap.
        let before = shine_opacity(0.5, 3.19);
        let after = shine_opacity(0.5, 3.21);
        assert!(before < 0.2, "pre-wrap opacity {before} should stay dim");
        assert!(after < 0.2, "post-wrap opacity {after} should stay dim");
    }

    #[test]
    fn entrance_reveal_is_monotonic_and_completes() {
        let start = entrance_progress(0.0);
        let first = entrance_progress(0.2);
        let second = entrance_progress(0.55);
        let end = entrance_progress(ENTRANCE_SECS);
        assert_eq!(start, 0.0);
        assert!(start < first && first < second && second < end);
        assert_eq!(end, 1.0);
        assert_eq!(entrance_progress(ENTRANCE_SECS + 10.0), 1.0);
    }
}
