//! Inline media: image/video viewer keys, playback state, media click
//! handling, and mermaid diagram affordances.

use super::{AgentView, AudioAnalysis, InlineAudioState, InlineVideoState};
use crate::app::app_view::InputOutcome;
use crate::render::SafeBuf;
use crate::terminal::overlay::{self, PostFlush};
use crate::theme::Theme;
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

const INLINE_MEDIA_SOURCE_MAX_BYTES: u64 = 32 * 1024 * 1024;
const INLINE_MEDIA_CACHE_MAX_BYTES: usize = 16 * 1024 * 1024;
const INLINE_MEDIA_ITEM_MAX_BYTES: usize = 8 * 1024 * 1024;
const INLINE_IMAGE_MAX_DIMENSION: u32 = 1280;

fn prepare_inline_media(path: &std::path::Path, is_video: bool) -> Option<Vec<u8>> {
    if std::fs::metadata(path).ok()?.len() > INLINE_MEDIA_SOURCE_MAX_BYTES {
        return None;
    }
    if is_video {
        let (frame, _, _) = crate::prompt_images::extract_poster_frame(path)?;
        let prepared = crate::terminal::image::prepare_overlay_image_bytes(&frame)?;
        return (prepared.len() <= INLINE_MEDIA_ITEM_MAX_BYTES).then_some(prepared);
    }

    use image::{ExtendedColorType, ImageEncoder};
    let image = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let image = image.thumbnail(INLINE_IMAGE_MAX_DIMENSION, INLINE_IMAGE_MAX_DIMENSION);
    let mut encoded = Vec::new();
    match crate::terminal::image::detect_graphics_protocol() {
        crate::terminal::image::GraphicsProtocol::Kitty => {
            use image::codecs::png::{CompressionType, FilterType, PngEncoder};
            let rgba = image.to_rgba8();
            PngEncoder::new_with_quality(&mut encoded, CompressionType::Fast, FilterType::Adaptive)
                .write_image(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    ExtendedColorType::Rgba8,
                )
                .ok()?;
        }
        crate::terminal::image::GraphicsProtocol::ITerm2 => {
            let rgb = image.to_rgb8();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 76)
                .write_image(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    ExtendedColorType::Rgb8,
                )
                .ok()?;
        }
        crate::terminal::image::GraphicsProtocol::None => return None,
    }
    (encoded.len() <= INLINE_MEDIA_ITEM_MAX_BYTES).then_some(encoded)
}

impl AgentView {
    // -- Image viewer input --------------------------------------------------

    /// Handle a key event in the image viewer modal.
    pub(super) fn handle_image_viewer_key(&mut self, key: &KeyEvent) -> InputOutcome {
        use crossterm::event::KeyCode;

        if self.image_viewer.is_none() {
            return InputOutcome::Unchanged;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // Clear the Kitty image before closing.
                // Old code bypassed STDERR_OUTPUT_LOCK which could interleave
                // mid-frame. Safe to revert: content is valid escapes, not raw text.
                xai_grok_shell::util::with_locked_stderr(|stderr| {
                    let clear = PostFlush::from(overlay::clear_kitty());
                    let _ = clear.write_to(stderr);
                });
                self.image_viewer = None;
                self.image_load_rx = None;
                // The viewer's decoded/re-encoded overlay image (tens of MB
                // for screenshots/renders) just dropped; input path, so a
                // synchronous purge lands between interactions.
                crate::memory_release::release_retained_memory_with("image-viewer-close");
            }
            _ => {}
        }
        InputOutcome::Changed
    }

    // -- Inline media rendering -----------------------------------------------

    /// Build Kitty/iTerm2 escape sequences for an inline media placement.
    pub(super) fn build_inline_media_escapes(
        &mut self,
        placement: &crate::scrollback::render::InlineMediaPlacement,
    ) -> Option<String> {
        use crate::prompt_images::decode_image_dimensions;

        let path = &placement.info.path;

        // During inline video playback, transmit the current frame.
        let is_video_playing = self.inline_video.as_ref().is_some_and(|v| v.path == *path);
        if is_video_playing {
            let vid_id = self.get_or_alloc_media_id(path);
            let video = self.inline_video.as_ref()?;
            let frame_data = video.viewer.current_frame_data();
            let (w, h) = decode_image_dimensions(frame_data)
                .unwrap_or((placement.info.width, placement.info.height));
            let transmit = crate::terminal::image::transmit_inline_image(frame_data, vid_id)?;
            let place = crate::terminal::image::place_inline_image(
                frame_data,
                w,
                h,
                placement.screen_rect,
                placement.full_rows,
                placement.top_crop_rows,
                vid_id,
                true,
            )?;
            return Some(format!("{transmit}{place}"));
        }

        // Static image or video poster frame.
        // Allocate the Kitty id only *after* bytes are in hand: a not-yet-written
        // path (or a failed read) must return `None` without recording an id, or
        // the next time the path is seen `needs_transmit` would be false and only
        // `place` (no `transmit`) would emit — leaving a blank image.
        let needs_transmit = !self.inline_media_ids.contains_key(path);
        let mut transmit_esc = String::new();

        if needs_transmit {
            // Preparation is serialized, bounded, and off-thread. It starts
            // only after model work is idle, so generated media cannot contend
            // with inference or block a TUI frame.
            if !self.inline_media_cache.contains_key(path) {
                if self.inline_media_load_failures.contains(path) {
                    return None;
                }
                if let Some(rx) = self.inline_media_loads.get(path) {
                    match rx.try_recv() {
                        Ok(Some(bytes)) => {
                            self.inline_media_loads.remove(path);
                            if !self.cache_inline_media_bytes(path.clone(), bytes) {
                                self.inline_media_load_failures.insert(path.clone());
                                return None;
                            }
                        }
                        Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            self.inline_media_loads.remove(path);
                            self.inline_media_load_failures.insert(path.clone());
                            return None;
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => return None,
                    }
                } else {
                    if !self.session.state.is_idle() || !self.inline_media_loads.is_empty() {
                        return None;
                    }
                    let (tx, rx) = std::sync::mpsc::sync_channel(1);
                    self.inline_media_loads.insert(path.clone(), rx);
                    let load_path = path.clone();
                    let is_video = placement.info.is_video;
                    std::thread::spawn(move || {
                        let _ = tx.send(prepare_inline_media(&load_path, is_video));
                    });
                    return None;
                }
                // Bound the cache: a long image-heavy session must not pin
                // every encoded image for its lifetime. Evicting drops only
                // CPU-side bytes — Kitty placements already transmitted stay
                // valid on the GPU (`inline_media_ids` is kept); an evicted
                // path re-reads from disk if it needs a re-transmit.
            }
            let image_id = self.get_or_alloc_media_id(path);
            let bytes = self.inline_media_cache.get(path)?;
            transmit_esc = crate::terminal::image::transmit_inline_image(bytes, image_id)?;
        }

        let image_id = self.get_or_alloc_media_id(path);
        let image_data = self.inline_media_cache.get(path)?;
        let (w, h) = decode_image_dimensions(image_data)
            .unwrap_or((placement.info.width, placement.info.height));

        // iTerm2 has no place-only escape — re-emit when placement moves.
        let emit_iterm = self
            .inline_media_iterm_emitted
            .get(path)
            .is_none_or(|last| *last != placement.screen_rect);
        let place_esc = crate::terminal::image::place_inline_image(
            image_data,
            w,
            h,
            placement.screen_rect,
            placement.full_rows,
            placement.top_crop_rows,
            image_id,
            emit_iterm,
        )?;
        if emit_iterm
            && crate::terminal::image::detect_graphics_protocol()
                == crate::terminal::image::GraphicsProtocol::ITerm2
        {
            self.inline_media_iterm_emitted
                .insert(path.clone(), placement.screen_rect);
        }

        Some(format!("{transmit_esc}{place_esc}"))
    }

    /// Paint each visible Mermaid affordance row (`◇ mermaid [Open Image]
    /// [Copy Image Path] [Copy Source]`) and register its click hit-rects.
    ///
    /// The leading `◇ mermaid` label is a dim, non-clickable marker. Every button
    /// is always clickable (`[Open]`/`[Copy path]` render lazily on click); a
    /// button whose hit-rect is under the mouse is highlighted, the rest are dim.
    /// A trailing dim `rendering…` hint follows the buttons while an on-click
    /// render for that diagram is in flight. The whole layout (label + button +
    /// hint columns) comes from
    /// [`affordance_row`](crate::scrollback::blocks::mermaid_content::affordance_row)
    /// so the painted labels and the hit-rects can't drift, and each segment is
    /// clipped to `screen_rect.width` (which excludes the timestamp reserve).
    pub(super) fn paint_diagram_affordances(
        &mut self,
        buf: &mut Buffer,
        placements: Vec<crate::scrollback::render::DiagramAffordancePlacement>,
        theme: &Theme,
    ) {
        use crate::scrollback::blocks::mermaid_content::affordance_row;
        use ratatui::style::Modifier;
        use unicode_width::UnicodeWidthStr;

        let (hover_col, hover_row) = self.last_mouse_pos;
        for aff in placements {
            let crate::scrollback::render::DiagramAffordancePlacement {
                screen_rect: rect,
                source,
            } = aff;
            // The transient `rendering…` hint shows only while an on-click render
            // for this diagram is in flight.
            let rendering = self.diagram_is_rendering(&source);
            let row = affordance_row(rendering);
            // A segment is drawn only if it fits wholly within the row width
            // (which already excludes the timestamp reserve), so labels never
            // spill past the content area and hit-rects stay inside the row.
            let fits =
                |col: u16, label: &str| col + UnicodeWidthStr::width(label) as u16 <= rect.width;

            // Leading dim, non-clickable `◇ mermaid` label.
            let (label_col, label_text) = row.label;
            if fits(label_col, label_text) {
                buf.set_string_safe(
                    rect.x.saturating_add(label_col),
                    rect.y,
                    label_text,
                    Style::default().fg(theme.gray_dim),
                );
            }

            // Register the diagram's source once — moved, not cloned (the
            // placement is owned and used only here) — when at least one button
            // fits; every fitting button below indexes into it for click routing.
            let source_idx = if row.buttons.iter().any(|b| fits(b.col, b.label)) {
                let idx = self.inline_media_hits.mermaid_sources.len();
                self.inline_media_hits.mermaid_sources.push(source);
                Some(idx)
            } else {
                None
            };
            for btn in row.buttons {
                if !fits(btn.col, btn.label) {
                    continue;
                }
                let bx = rect.x.saturating_add(btn.col);
                let width = UnicodeWidthStr::width(btn.label) as u16;
                let hit = Rect {
                    x: bx,
                    y: rect.y,
                    width,
                    height: 1,
                };
                // Hovered button is highlighted; idle buttons stay at the normal
                // `gray` (brighter than the dim `◇ mermaid` label) so they remain
                // discoverable at rest.
                let style = if hit.contains((hover_col, hover_row).into()) {
                    Style::default()
                        .fg(theme.text_primary)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    Style::default().fg(theme.gray)
                };
                buf.set_string_safe(bx, rect.y, btn.label, style);
                if let Some(idx) = source_idx {
                    self.inline_media_hits
                        .mermaid_buttons
                        .push((hit, btn.kind, idx));
                }
            }

            // Trailing dim `rendering…` hint after the buttons (not clickable).
            if let Some((col, status)) = row.status
                && fits(col, status)
            {
                buf.set_string_safe(
                    rect.x.saturating_add(col),
                    rect.y,
                    status,
                    Style::default().fg(theme.gray_dim),
                );
            }
        }
    }

    /// Whether the diagram with `source` has an on-click render in flight (drives
    /// the affordance row's transient `rendering…` hint).
    fn diagram_is_rendering(&self, source: &str) -> bool {
        self.mermaid_is_rendering(source)
    }

    /// Get or allocate a Kitty image ID for the given media path.
    fn get_or_alloc_media_id(&mut self, path: &std::path::Path) -> u32 {
        if let Some(&id) = self.inline_media_ids.get(path) {
            return id;
        }
        let id = self.next_inline_media_id;
        self.next_inline_media_id += 1;
        self.inline_media_ids.insert(path.to_path_buf(), id);
        id
    }

    fn cache_inline_media_bytes(&mut self, path: std::path::PathBuf, bytes: Vec<u8>) -> bool {
        let incoming = bytes.len();
        if incoming > INLINE_MEDIA_CACHE_MAX_BYTES {
            return false;
        }
        let mut total = self
            .inline_media_cache
            .values()
            .map(Vec::len)
            .sum::<usize>()
            + incoming;
        while total > INLINE_MEDIA_CACHE_MAX_BYTES {
            let Some(victim) = self.inline_media_cache.keys().next().cloned() else {
                break;
            };
            if let Some(evicted) = self.inline_media_cache.remove(&victim) {
                total -= evicted.len();
            }
        }
        self.inline_media_cache.insert(path, bytes);
        true
    }

    /// Drain one completed lazy preview job. This keeps invisible or
    /// scrolled-away media from holding the slow-tick gate open indefinitely.
    pub(crate) fn poll_inline_media_loads(&mut self) -> bool {
        let completed = self
            .inline_media_loads
            .iter()
            .find_map(|(path, rx)| match rx.try_recv() {
                Ok(result) => Some((path.clone(), result)),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some((path.clone(), None)),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
            });
        let Some((path, result)) = completed else {
            return false;
        };
        self.inline_media_loads.remove(&path);
        match result {
            Some(bytes) => {
                if !self.cache_inline_media_bytes(path.clone(), bytes) {
                    self.inline_media_load_failures.insert(path);
                }
            }
            None => {
                self.inline_media_load_failures.insert(path);
            }
        }
        true
    }

    /// Drain this agent's inline-media placement tracking and return the
    /// Kitty delete escapes for every image it has placed on the GPU.
    ///
    /// Kitty graphics are independent of the cell grid: they survive
    /// redraws until explicitly deleted, and every regular clear path
    /// lives inside [`AgentView::draw`]. When another view takes over the
    /// frame (e.g. the agent dashboard), those per-frame clears stop
    /// running, so the caller uses this to delete whatever this agent
    /// left on screen. Resetting `inline_media_ids` forces a fresh
    /// transmit when this agent next draws; any active inline playback
    /// is stopped, mirroring the scrolled-off-screen clear path.
    ///
    /// Returns `None` when this agent (and its subagent views) has no
    /// placements.
    pub(crate) fn take_inline_media_clear_escapes(&mut self) -> Option<String> {
        let mut clear_esc = self
            .take_own_inline_media_clear_escapes()
            .unwrap_or_default();
        if let Some(esc) = self.take_subagent_inline_media_clear_escapes() {
            clear_esc.push_str(&esc);
        }
        (!clear_esc.is_empty()).then_some(clear_esc)
    }

    /// This view's own placements only, leaving `subagent_views` untouched.
    /// Used by the fullscreen-subagent takeover in [`AgentView::draw`]: the
    /// parent's images must be deleted, but the child is about to draw and
    /// manages its own placements — draining it too would just force a
    /// re-transmit.
    pub(super) fn take_own_inline_media_clear_escapes(&mut self) -> Option<String> {
        // Also proceed when only playback state remains (`inline_video` Some
        // with no active placements — e.g. frames finished loading after the
        // media scrolled off): the drain must still stop the ticking video,
        // or it keeps holding the animation gate open invisibly and its
        // eventual drop is never purged.
        if !self.inline_media_active
            && self.inline_media_ids.is_empty()
            && self.inline_video.is_none()
        {
            return None;
        }
        self.inline_media_active = false;
        self.stop_inline_playback();
        let mut clear_esc = String::new();
        for &id in self.inline_media_ids.values() {
            clear_esc.push_str(&crate::terminal::image::clear_kitty_image(id));
        }
        self.inline_media_ids.clear();
        self.inline_media_iterm_emitted.clear();
        self.last_placed_ids.clear();
        (!clear_esc.is_empty()).then_some(clear_esc)
    }

    /// Stop inline video playback, cancelling its bounded decoder and request
    /// a post-draw allocator purge. Returns whether a
    /// video was actually playing — callers on the draw path rely on the
    /// deferred request (never a synchronous purge mid-frame), and image-only
    /// paths (`None` here) must not purge at all.
    pub(super) fn stop_inline_playback(&mut self) -> bool {
        let had_video = self.inline_video.take().is_some();
        if had_video {
            crate::memory_release::request_release_after_draw_with("inline-video-stop");
        }
        had_video
    }

    /// Install a freshly-started bounded inline decoder, dropping (and
    /// requesting a post-draw purge for) any previous playback state.
    pub(crate) fn replace_inline_video(&mut self, video: crate::app::agent_view::InlineVideoState) {
        if self.inline_video.replace(video).is_some() {
            // Switching videos: the previous frame set just dropped.
            crate::memory_release::request_release_after_draw_with("inline-video-replace");
        }
    }

    /// Subagent fullscreen views render inline media with their own ids —
    /// drain those (recursively), leaving this view's placements alone.
    pub(super) fn take_subagent_inline_media_clear_escapes(&mut self) -> Option<String> {
        let mut clear_esc = String::new();
        for child in self.subagent_views.values_mut() {
            if let Some(esc) = child.take_inline_media_clear_escapes() {
                clear_esc.push_str(&esc);
            }
        }
        (!clear_esc.is_empty()).then_some(clear_esc)
    }

    /// Refresh [`Self::media_link_paths`] — the absolute paths of media
    /// generated in this transcript — from scrollback, but only when its
    /// generation has changed. The model prints short session-relative paths
    /// (`images/1.jpg`); resolving them against the actual generated files ties
    /// each link to the file its message produced (correct across forks) and
    /// never opens an out-of-session or arbitrary file.
    pub(crate) fn ensure_media_link_paths(&mut self) {
        let generation = self.scrollback.generation();
        if self.media_link_paths_gen == Some(generation) {
            return;
        }
        self.media_link_paths_gen = Some(generation);
        self.media_link_paths.clear();
        self.media_link_paths.extend(
            self.scrollback
                .iter_entries()
                .filter_map(|(_, entry)| entry.block.media_ref_path()),
        );
    }

    /// Open a media file in the OS-native default application (Preview,
    /// default video player, etc.). Shared by the `[Open]` button, the
    /// inline-image click target, and the Enter-key handler.
    pub(crate) fn open_media_natively(&mut self, path: &std::path::Path) -> bool {
        if crate::app::link_opener::open_path(path) {
            self.show_toast("Opening in default app\u{2026}");
            true
        } else {
            self.show_toast("Could not open file");
            false
        }
    }

    /// Start, pause, or resume inline video playback. A completed clip restarts
    /// from the beginning. Decoding stays in the bounded background pipeline.
    pub(crate) fn start_inline_video_playback(&mut self, path: &std::path::Path) {
        // Existing playback for this path acts as a normal play/pause toggle.
        if let Some(ref mut video) = self.inline_video
            && video.path == path
        {
            if video.viewer.finished {
                video.viewer.restart();
            } else {
                video.viewer.toggle_play_pause();
            }
            return;
        }
        if !self.session.state.is_idle() {
            self.show_toast("Media preview is available when the response completes");
            return;
        }
        // Start ffmpeg and receive the first frame in a background thread.
        let path_owned = path.to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();
        self.video_load_rx = Some(rx);
        self.show_toast("Loading video\u{2026}");
        std::thread::spawn(move || {
            let result =
                crate::prompt_images::VideoViewerState::open_from_path(&path_owned).map(|viewer| {
                    InlineVideoState {
                        path: path_owned,
                        viewer,
                    }
                });
            let _ = tx.send(result);
        });
    }

    /// Toggle audio playback for a generated music or speech artifact.
    /// `ffplay` keeps decoding outside the TUI process; when it is unavailable
    /// the OS-native player is opened as the graceful fallback.
    pub(crate) fn toggle_inline_audio_playback(&mut self, path: &std::path::Path) {
        if !self.session.state.is_idle() {
            self.show_toast("Media playback is available when the response completes");
            return;
        }

        if let Some(audio) = self.inline_audio.as_mut()
            && audio.path == path
        {
            if audio.is_playing() {
                pause_audio(audio);
                self.show_toast("Audio paused");
            } else {
                if audio
                    .duration_secs
                    .is_some_and(|duration| audio.position_at_start_secs >= duration - 0.1)
                {
                    audio.position_at_start_secs = 0.0;
                }
                if restart_audio(audio) {
                    self.show_toast("Playing audio");
                } else {
                    self.open_media_natively(path);
                }
            }
            return;
        }

        self.inline_audio = None;
        match spawn_audio_child(path, 0.0, 80) {
            Ok(child) => {
                let (analysis_tx, analysis_rx) = std::sync::mpsc::sync_channel(1);
                let analysis_cancel =
                    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let worker_cancel = std::sync::Arc::clone(&analysis_cancel);
                let analysis_path = path.to_path_buf();
                std::thread::spawn(move || {
                    let _ = analysis_tx.send(analyze_audio(&analysis_path, &worker_cancel));
                });
                self.inline_audio = Some(InlineAudioState {
                    path: path.to_path_buf(),
                    child: Some(child),
                    position_at_start_secs: 0.0,
                    started_at: Some(std::time::Instant::now()),
                    duration_secs: None,
                    volume_percent: 80,
                    waveform: Vec::new(),
                    analysis_rx: Some(analysis_rx),
                    analysis_cancel: Some(analysis_cancel),
                });
                self.show_toast("Playing audio");
            }
            Err(error) => {
                tracing::debug!(%error, "ffplay unavailable; opening audio natively");
                self.open_media_natively(path);
            }
        }
    }

    /// Reap naturally completed audio playback. Returns `true` when the card
    /// needs repainting from Stop back to Play.
    pub(crate) fn poll_inline_audio(&mut self) -> bool {
        let Some(audio) = self.inline_audio.as_mut() else {
            return false;
        };
        let mut changed = false;
        if !self.session.state.is_idle()
            && let Some(cancel) = audio.analysis_cancel.as_ref()
        {
            cancel.store(true, std::sync::atomic::Ordering::Release);
        }
        if let Some(rx) = audio.analysis_rx.as_ref() {
            match rx.try_recv() {
                Ok(analysis) => {
                    audio.duration_secs = analysis.duration_secs;
                    audio.waveform = analysis.waveform;
                    audio.analysis_rx = None;
                    audio.analysis_cancel = None;
                    changed = true;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    audio.analysis_rx = None;
                    audio.analysis_cancel = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        let finished = audio
            .child
            .as_mut()
            .is_some_and(|child| matches!(child.try_wait(), Ok(Some(_)) | Err(_)));
        if finished {
            audio.child = None;
            audio.started_at = None;
            audio.position_at_start_secs = audio.duration_secs.unwrap_or(0.0);
            changed = true;
        }
        changed
    }

    pub(crate) fn seek_inline_audio(&mut self, path: &std::path::Path, delta_secs: i32) {
        let Some(audio) = self
            .inline_audio
            .as_mut()
            .filter(|audio| audio.path == path)
        else {
            return;
        };
        let was_playing = audio.is_playing();
        let target = (audio.position_secs() + delta_secs as f64).max(0.0);
        pause_audio(audio);
        audio.position_at_start_secs = audio
            .duration_secs
            .map_or(target, |duration| target.min(duration));
        if was_playing {
            let _ = restart_audio(audio);
        }
    }

    pub(crate) fn adjust_inline_audio_volume(&mut self, path: &std::path::Path, delta: i8) {
        let Some(audio) = self
            .inline_audio
            .as_mut()
            .filter(|audio| audio.path == path)
        else {
            return;
        };
        let was_playing = audio.is_playing();
        let position = audio.position_secs();
        pause_audio(audio);
        audio.position_at_start_secs = position;
        audio.volume_percent = (audio.volume_percent as i16 + delta as i16).clamp(0, 100) as u8;
        if was_playing {
            let _ = restart_audio(audio);
        }
    }

    pub(crate) fn seek_inline_video(&mut self, path: &std::path::Path, delta_secs: i32) {
        let Some(video) = self
            .inline_video
            .as_mut()
            .filter(|video| video.path == path)
        else {
            return;
        };
        if delta_secs < 0 {
            for _ in 0..delta_secs.unsigned_abs() {
                video.viewer.seek_backward();
            }
        } else {
            for _ in 0..delta_secs as u32 {
                video.viewer.seek_forward();
            }
        }
    }

    // -- Inline media click handling -----------------------------------------

    /// Handle a click on inline media buttons. Returns `Some(InputOutcome)` if
    /// the click was consumed, `None` to fall through to normal handling.
    pub(in crate::app) fn handle_inline_media_click(
        &mut self,
        col: u16,
        row: u16,
    ) -> Option<InputOutcome> {
        let pos = ratatui::layout::Position::new(col, row);

        // [Open] button or inline image → open natively. Checked before the
        // play targets so a video's [Open] button opens rather than plays.
        let open_target = self
            .inline_media_hits
            .open_buttons
            .iter()
            .chain(self.inline_media_hits.media_areas.iter())
            .find(|(rect, _)| rect.contains(pos))
            .map(|(_, path)| path.clone());
        if let Some(path) = open_target {
            self.open_media_natively(&path);
            return Some(InputOutcome::Changed);
        }

        if let Some(path) = self
            .inline_media_hits
            .audio_play_buttons
            .iter()
            .find(|(rect, _)| rect.contains(pos))
            .map(|(_, path)| path.clone())
        {
            self.toggle_inline_audio_playback(&path);
            return Some(InputOutcome::Changed);
        }

        if let Some((path, delta)) = self
            .inline_media_hits
            .audio_seek_buttons
            .iter()
            .find(|(rect, _, _)| rect.contains(pos))
            .map(|(_, path, delta)| (path.clone(), *delta))
        {
            self.seek_inline_audio(&path, delta);
            return Some(InputOutcome::Changed);
        }

        if let Some((path, delta)) = self
            .inline_media_hits
            .video_seek_buttons
            .iter()
            .find(|(rect, _, _)| rect.contains(pos))
            .map(|(_, path, delta)| (path.clone(), *delta))
        {
            self.seek_inline_video(&path, delta);
            return Some(InputOutcome::Changed);
        }

        if let Some((path, delta)) = self
            .inline_media_hits
            .audio_volume_buttons
            .iter()
            .find(|(rect, _, _)| rect.contains(pos))
            .map(|(_, path, delta)| (path.clone(), *delta))
        {
            self.adjust_inline_audio_volume(&path, delta);
            return Some(InputOutcome::Changed);
        }

        // [Play] button or video poster → start/restart inline playback.
        let play_target = self
            .inline_media_hits
            .play_buttons
            .iter()
            .chain(self.inline_media_hits.video_play_areas.iter())
            .find(|(rect, _)| rect.contains(pos))
            .map(|(_, path)| path.clone());
        if let Some(path) = play_target {
            self.start_inline_video_playback(&path);
            return Some(InputOutcome::Changed);
        }

        // [Copy] button → copy image to clipboard (async).
        if let Some((_, path)) = self
            .inline_media_hits
            .copy_image_buttons
            .iter()
            .find(|(rect, _)| rect.contains(pos))
        {
            let path = path.clone();
            std::thread::spawn(move || {
                if let Err(e) = xai_grok_shell::util::clipboard::set_image_file(&path) {
                    tracing::debug!("copy image failed: {e}");
                }
            });
            self.show_toast("Copied image");
            return Some(InputOutcome::Changed);
        }

        // Click on filepath line → copy path to clipboard.
        if let Some((_, path)) = self
            .inline_media_hits
            .filepath_areas
            .iter()
            .find(|(rect, _)| rect.contains(pos))
        {
            let path_str = path.display().to_string();
            self.copy_to_clipboard(&path_str);
            return Some(InputOutcome::Changed);
        }

        // Mermaid affordance row → render-on-click (Open/Copy path) or copy
        // source. Resolve the kind + source index first so the `mermaid_buttons`
        // borrow ends before the `&mut self` dispatch below.
        let mermaid_hit = self
            .inline_media_hits
            .mermaid_buttons
            .iter()
            .find(|(rect, _, _)| rect.contains(pos))
            .map(|&(_, kind, idx)| (kind, idx));
        if let Some((kind, idx)) = mermaid_hit {
            let source = self
                .inline_media_hits
                .mermaid_sources
                .get(idx)
                .cloned()
                .unwrap_or_default();
            self.on_mermaid_affordance_click(kind, source);
            return Some(InputOutcome::Changed);
        }

        None
    }

    /// Route a Mermaid affordance-row click. `[Copy source]` copies the diagram
    /// source (no render); `[Open]`/`[Copy path]` render it lazily at the live
    /// theme/width and then open the PNG / copy its path. `source` is moved into
    /// the renderer, never cloned. `copy_to_clipboard` owns the copy toast.
    fn on_mermaid_affordance_click(
        &mut self,
        kind: crate::scrollback::blocks::mermaid_content::AffordanceKind,
        source: String,
    ) {
        use crate::scrollback::blocks::mermaid_content::AffordanceKind;
        match kind {
            AffordanceKind::CopySource => {
                if self.copy_to_clipboard(&source).is_failed() {
                    crate::unified_log::error(
                        "mermaid.copy_source.failed",
                        self.session.session_id.as_ref().map(|s| s.0.as_ref()),
                        Some(serde_json::json!({ "source_len": source.len() })),
                    );
                }
            }
            AffordanceKind::Open | AffordanceKind::CopyPath => {
                let action = if matches!(kind, AffordanceKind::Open) {
                    crate::app::mermaid_worker::MermaidClickAction::Open
                } else {
                    crate::app::mermaid_worker::MermaidClickAction::CopyPath
                };
                self.request_mermaid_render(source, action);
            }
        }
    }

    // -- Video viewer input --------------------------------------------------

    /// Handle a key event in the video viewer modal.
    pub(super) fn handle_video_viewer_key(&mut self, key: &KeyEvent) -> InputOutcome {
        use crossterm::event::KeyCode;

        let Some(ref mut viewer) = self.video_viewer else {
            return InputOutcome::Unchanged;
        };

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // Clear the Kitty image before closing.
                xai_grok_shell::util::with_locked_stderr(|stderr| {
                    let clear = PostFlush::from(overlay::clear_kitty());
                    let _ = clear.write_to(stderr);
                });
                self.video_viewer = None;
                // The viewer's pre-extracted frame set (~50–300 MB for a
                // typical clip) just dropped; return the pages to the OS.
                crate::memory_release::release_retained_memory_with("video-viewer-close");
            }
            KeyCode::Char(' ') => {
                viewer.toggle_play_pause();
            }
            KeyCode::Right | KeyCode::Char('l') => {
                viewer.seek_forward();
            }
            KeyCode::Left | KeyCode::Char('h') => {
                viewer.seek_backward();
            }
            _ => {}
        }
        InputOutcome::Changed
    }

    // -- /gboom easter egg input ------------------------------------------------

    /// Handle a key event in the `/gboom` game modal.
    pub(super) fn handle_gboom_key(&mut self, key: &KeyEvent) -> InputOutcome {
        let Some(ref mut gboom) = self.gboom else {
            return InputOutcome::Unchanged;
        };
        match gboom.handle_key(key) {
            crate::gboom::GboomKeyOutcome::Close => {
                // Clear the kitty image before closing (same as the video
                // viewer) so no stale frame lingers in the cell grid.
                xai_grok_shell::util::with_locked_stderr(|stderr| {
                    let clear = PostFlush::from(overlay::clear_kitty());
                    let _ = clear.write_to(stderr);
                });
                self.gboom = None;
            }
            crate::gboom::GboomKeyOutcome::Changed => {}
        }
        InputOutcome::Changed
    }

    /// Handle a key-release in the `/gboom` modal (un-latch movement).
    pub(super) fn handle_gboom_release(&mut self, key: &KeyEvent) -> InputOutcome {
        if let Some(ref mut gboom) = self.gboom {
            gboom.handle_release(key);
        }
        InputOutcome::Changed
    }

    pub(super) fn handle_gboom_mouse(&mut self, mouse: &MouseEvent) -> InputOutcome {
        if let Some(ref mut gboom) = self.gboom {
            gboom.handle_mouse(mouse);
        }
        InputOutcome::Changed
    }
}

fn spawn_audio_child(
    path: &std::path::Path,
    position_secs: f64,
    volume_percent: u8,
) -> std::io::Result<std::process::Child> {
    let mut command = std::process::Command::new("ffplay");
    command
        .args([
            "-nodisp",
            "-autoexit",
            "-loglevel",
            "quiet",
            "-threads",
            "1",
        ])
        .args(["-ss", &format!("{position_secs:.3}")])
        .args(["-volume", &volume_percent.to_string()])
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }
    command.spawn()
}

fn pause_audio(audio: &mut InlineAudioState) {
    let position = audio.position_secs();
    if let Some(mut child) = audio.child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    audio.position_at_start_secs = position;
    audio.started_at = None;
}

fn restart_audio(audio: &mut InlineAudioState) -> bool {
    match spawn_audio_child(
        &audio.path,
        audio.position_at_start_secs,
        audio.volume_percent,
    ) {
        Ok(child) => {
            audio.child = Some(child);
            audio.started_at = Some(std::time::Instant::now());
            true
        }
        Err(error) => {
            tracing::debug!(%error, "ffplay unavailable while resuming audio");
            false
        }
    }
}

fn analyze_audio(path: &std::path::Path, cancel: &std::sync::atomic::AtomicBool) -> AudioAnalysis {
    let duration_secs = probe_audio_duration(path);
    let waveform = decode_audio_waveform(path, cancel);
    AudioAnalysis {
        duration_secs,
        waveform,
    }
}

fn probe_audio_duration(path: &std::path::Path) -> Option<f64> {
    let mut command = std::process::Command::new("ffprobe");
    command
        .args([
            "-v",
            "quiet",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    xai_tty_utils::detach_std_command(&mut command);
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().parse().ok())
        .flatten()
}

fn decode_audio_waveform(
    path: &std::path::Path,
    cancel: &std::sync::atomic::AtomicBool,
) -> Vec<u8> {
    use std::io::Read;
    use std::sync::atomic::Ordering;

    const WAVEFORM_WIDTH: usize = 64;
    const WAVEFORM_HEIGHT: usize = 8;
    if cancel.load(Ordering::Acquire) {
        return Vec::new();
    }
    let mut command = std::process::Command::new("ffmpeg");
    command
        .args(["-hide_banner", "-loglevel", "error", "-threads", "1", "-i"])
        .arg(path)
        .args([
            "-t",
            "300",
            "-filter_complex",
            "aformat=channel_layouts=mono,showwavespic=s=64x8:colors=white",
            "-frames:v",
            "1",
            "-pix_fmt",
            "gray",
            "-f",
            "rawvideo",
            "-",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    xai_tty_utils::detach_std_command(&mut command);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }
    let Ok(mut child) = command.spawn() else {
        return Vec::new();
    };
    let status = loop {
        if cancel.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            return Vec::new();
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(_) => return Vec::new(),
        }
    };
    if !status.success() {
        return Vec::new();
    }
    let mut pixels = Vec::with_capacity(WAVEFORM_WIDTH * WAVEFORM_HEIGHT);
    if child
        .stdout
        .take()
        .is_none_or(|mut stdout| stdout.read_to_end(&mut pixels).is_err())
        || pixels.len() < WAVEFORM_WIDTH * WAVEFORM_HEIGHT
    {
        return Vec::new();
    }
    (0..WAVEFORM_WIDTH)
        .map(|column| {
            let lit = (0..WAVEFORM_HEIGHT)
                .filter(|row| pixels[row * WAVEFORM_WIDTH + column] > 0)
                .count();
            lit.saturating_sub(1).min(7) as u8
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::memory_release::test_support;

    fn make_agent() -> crate::app::agent_view::AgentView {
        crate::test_util::make_agent_view(None, "/tmp")
    }

    fn stub_inline_video() -> crate::app::agent_view::InlineVideoState {
        crate::app::agent_view::InlineVideoState {
            path: std::path::PathBuf::from("/tmp/clip.mp4"),
            viewer: crate::prompt_images::VideoViewerState::test_stub(),
        }
    }

    /// Closing the video viewer modal drops the pre-extracted frame set —
    /// the purge must fire on close and never on other viewer keys.
    #[test]
    fn video_viewer_close_releases_retained_memory() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        test_support::install_counting_hook();

        let mut agent = make_agent();
        agent.video_viewer = Some(crate::prompt_images::VideoViewerState::test_stub());

        // A non-close key keeps the viewer (and its frames) → no purge.
        let before = test_support::calls();
        agent.handle_video_viewer_key(&KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(agent.video_viewer.is_some());
        assert_eq!(
            test_support::calls(),
            before,
            "play/pause drops nothing and must not purge"
        );

        // Esc closes → frames drop → one purge.
        let before = test_support::calls();
        agent.handle_video_viewer_key(&KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(agent.video_viewer.is_none());
        assert_eq!(
            test_support::calls(),
            before + 1,
            "closing the viewer must purge after the frame set drops"
        );
    }

    /// Draining inline-media placements requests a POST-DRAW purge only when
    /// live playback (a frame set) was actually dropped — image-only clears
    /// must not, and the purge must never run synchronously (these paths sit
    /// inside `draw`). Serialized: the deferred-request flag is process-wide.
    #[test]
    #[serial_test::serial(MEMORY_RELEASE_DEFER)]
    fn inline_media_clear_defers_release_only_for_video() {
        test_support::install_counting_hook();
        // Drain any stale request left by an earlier test in this group.
        crate::memory_release::run_deferred_release();

        let mut agent = make_agent();

        // Image-only placements active: clear drops no frames → no request.
        agent.inline_media_active = true;
        let before = test_support::calls();
        let _ = agent.take_inline_media_clear_escapes();
        crate::memory_release::run_deferred_release();
        assert_eq!(
            test_support::calls(),
            before,
            "an image-only media clear must not purge"
        );

        // Active inline playback: sync no purge; the drain runs it → one.
        agent.inline_media_active = true;
        agent.inline_video = Some(stub_inline_video());
        let before = test_support::calls();
        let _ = agent.take_inline_media_clear_escapes();
        assert!(agent.inline_video.is_none());
        assert_eq!(
            test_support::calls(),
            before,
            "draw-path video stop must never purge synchronously"
        );
        crate::memory_release::run_deferred_release();
        assert_eq!(
            test_support::calls(),
            before + 1,
            "the post-draw drain must purge the dropped frame set"
        );

        // Orphaned playback (frames finished loading after the media
        // scrolled off: no active flag, no placements): the drain must still
        // stop the video and request its purge.
        agent.inline_media_active = false;
        agent.inline_video = Some(stub_inline_video());
        let before = test_support::calls();
        assert!(agent.take_inline_media_clear_escapes().is_none());
        assert!(
            agent.inline_video.is_none(),
            "orphaned playback must be stopped by the drain"
        );
        crate::memory_release::run_deferred_release();
        assert_eq!(test_support::calls(), before + 1);

        // Nothing at all: the early no-placement return → no request.
        let before = test_support::calls();
        let _ = agent.take_inline_media_clear_escapes();
        crate::memory_release::run_deferred_release();
        assert_eq!(
            test_support::calls(),
            before,
            "a no-op clear must not purge"
        );
    }

    /// Installing freshly-extracted frames purges the PREVIOUS playback's
    /// frame set (deferred), and never purges on first install.
    #[test]
    #[serial_test::serial(MEMORY_RELEASE_DEFER)]
    fn replace_inline_video_defers_release_only_when_replacing() {
        test_support::install_counting_hook();
        crate::memory_release::run_deferred_release();

        let mut agent = make_agent();

        // First install: nothing drops → no request.
        let before = test_support::calls();
        agent.replace_inline_video(stub_inline_video());
        crate::memory_release::run_deferred_release();
        assert_eq!(
            test_support::calls(),
            before,
            "first frame-set install drops nothing and must not purge"
        );

        // Replacement: the old frame set drops → deferred purge.
        let before = test_support::calls();
        agent.replace_inline_video(stub_inline_video());
        assert_eq!(
            test_support::calls(),
            before,
            "tick-path replacement must never purge synchronously"
        );
        crate::memory_release::run_deferred_release();
        assert_eq!(test_support::calls(), before + 1);
    }

    /// Closing the image viewer drops the decoded overlay image — purge
    /// synchronously (input path), exactly once.
    #[test]
    fn image_viewer_close_releases_retained_memory() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        test_support::install_counting_hook();

        let mut agent = make_agent();
        agent.image_viewer = Some(
            crate::prompt_images::ImageViewerState::open_from_path_deferred(std::path::Path::new(
                "x.png",
            )),
        );
        let before = test_support::calls();
        agent.handle_image_viewer_key(&KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(agent.image_viewer.is_none());
        assert_eq!(
            test_support::calls(),
            before + 1,
            "closing the image viewer must purge after the image drops"
        );
    }
}
