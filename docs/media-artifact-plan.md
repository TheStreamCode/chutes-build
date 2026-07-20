# MediaArtifact Plan

## Implementation status (2026-07-19)

Phases 1 and 2 are implemented for new Chutes `generate_media` results: the
versioned artifact is serialized through the tool output and ACP, the pager
routes it without prose scraping, and existing image/video behavior remains as
a compatibility fallback for old sessions. Unknown future kinds deserialize to
a generic local-file card.

Phases 3 and 4 are implemented for the current terminal preview path. Music and
speech cards expose local pause/resume, ten-second seek, volume, elapsed/total
time, and a lazily computed waveform using `ffplay`, `ffprobe`, and FFmpeg. The
child process is cleaned up with view state, media never autoplays, and the
operating-system player remains the fallback. Video decoding keeps one visible
frame plus a bounded two-frame queue and cancels the single-threaded decoder on
pause/drop. Inline playback exposes play/pause and one-second seek controls and
auto-pauses if a new inference begins. Static image/poster preparation is
off-thread, serialized, bounded, and deferred until model inference is idle.
Waveform analysis is cancellable when inference resumes. Broader
terminal-protocol coverage remains planned.

## Objective

Replace extension- and prose-based media detection with one typed artifact
contract shared by Chutes generation tools, ACP transport, session persistence,
scrollback, and the TUI. The contract must support present and future image,
video, audio, document, and generated-media formats without coupling core logic
to a particular model family.

## Remaining constraints

- Legacy image/video sessions still use their previous typed outputs or
  text/path compatibility extraction.
- Image preview and silent video-frame playback require a supported terminal
  graphics protocol; unsupported or oversized media keeps the text card and
  native-open action.
- In-TUI audio controls require local FFmpeg tools. Native open remains the
  fallback for a missing executable or unsupported codec.
- Waveform analysis samples at low rate, uses one FFmpeg thread, is capped at
  five minutes, and begins only after the user requests playback.

## Domain contract

Introduce a shared, versioned `MediaArtifact` type in a provider-neutral crate:

```rust
pub struct MediaArtifact {
    pub schema_version: u16,
    pub id: ArtifactId,
    pub kind: MediaKind,
    pub mime_type: String,
    pub local_path: PathBuf,
    pub byte_len: u64,
    pub dimensions: Option<MediaDimensions>,
    pub duration_ms: Option<u64>,
    pub has_audio: Option<bool>,
    pub preview: Option<MediaPreview>,
    pub provenance: Option<MediaProvenance>,
}
```

`MediaKind` starts with `Image`, `Video`, `Audio`, and `Document`, plus
`Unknown` for forward compatibility. Behavior is selected primarily from a
validated MIME type and decoded metadata; filename extensions are display and
fallback hints only.

## Ownership boundaries

- **Chutes media client:** validates the response, writes the file and sidecar,
  probes minimal metadata, and returns `MediaArtifact`.
- **Tool protocol:** adds a typed media-artifact output variant. The model sees
  a concise textual receipt; ACP receives the structured artifact unchanged.
- **Session storage:** persists artifact metadata, not media bytes, and keeps
  paths session/workspace scoped.
- **Pager:** converts an artifact into a media card and chooses an interaction
  strategy from terminal and decoder capabilities.
- **Playback backends:** own decoding and playback handles; they never leak
  provider-specific data into rendering state.

## TUI behavior

### Images

- Inline preview when the terminal capability probe succeeds.
- Open, copy image, copy path, fit, and full-preview actions.
- Text card with dimensions, format, size, and `[Open]` fallback otherwise.

### Video

- Immediate poster frame and metadata card.
- User-initiated inline preview with play/pause and seek controls.
- Rolling decode buffer instead of extracting the entire clip into memory.
- `[Open]` remains available for full-resolution playback, sound, fullscreen,
  and unsupported codecs.

### Audio and music

- Local play/pause, stop, seek, elapsed/total time, and volume controls.
- Lazily generated waveform or compact level visualization.
- Background playback state independent from TUI redraw frequency.
- `[Open]` fallback when the native audio backend or codec is unavailable.

No generated media should autoplay.

## Capability negotiation

Replace the current operating-system blanket with an explicit runtime matrix:

- terminal graphics protocol and required placement operations;
- ConPTY version/capability where applicable;
- FFmpeg/FFprobe availability;
- audio output backend and codec availability;
- minimal/scrollback-native mode;
- reduced-motion and user preview preferences.

Unsupported combinations degrade to a complete text card and native open
action. Capability failure must never leave blank reserved rows or stale image
placements.

## Performance and lifecycle

- Probe metadata and generate previews in background tasks.
- Use bounded LRU caches for poster frames, thumbnails, and decoded video
  frames; cache keys include the artifact identity and content metadata.
- Decode video into a small ahead/behind ring buffer and drop frames after
  playback stops or the item leaves the active viewport.
- Keep audio decoding off the render loop and communicate through bounded
  control/status channels.
- Cancel obsolete loads when a session, modal, or artifact closes.
- Expose local counters for decode latency, cache bytes, dropped frames, and
  playback underruns without collecting media content.

## Security and privacy

- Canonicalize every path and enforce the workspace/session output boundary.
- Verify magic bytes and decoded media metadata against the claimed MIME type.
- Treat filenames, metadata, captions, and provenance as untrusted text before
  rendering them in a terminal.
- Never pass untrusted values through shell command strings.
- Keep generated files, previews, and playback local; no thumbnail or codec
  service may receive them.
- Apply byte, pixel, duration, frame-rate, and decode-time limits before
  allocating large buffers.

## Migration phases

### Phase 1: typed contract

Add `MediaArtifact`, typed tool output, ACP conversion, persistence round trips,
and adapters for existing image/video generation variants. Keep the old parsing
path as a compatibility fallback for older sessions.

### Phase 2: unified media cards

Render all generated Chutes media through one scrollback block. Preserve the
current image and video behavior while removing prose/path scraping for new
outputs.

### Phase 3: audio player

Add a cross-platform local playback backend, controls, state cleanup, and
waveform generation. Gate codecs by actual decoder support and retain `[Open]`.

Status: implemented for the local FFmpeg/FFplay backend with native-open
fallback.

### Phase 4: streaming video preview

Replace full upfront frame extraction with the bounded rolling decoder. Add
poster caching, frame-drop pacing, resize handling, and memory-pressure tests.

Status: the rolling decoder, cancellation, single-thread limit, bounded queue,
and bounded lazy poster/image cache are implemented. Dedicated resize and
memory-pressure test coverage remains to be expanded.

### Phase 5: terminal coverage

Introduce capability probes and tested Kitty/Sixel/iTerm fallback behavior for
the supported terminal matrix, including modern Windows ConPTY and VS Code.

## Verification

- Serialization compatibility for current and unknown artifact kinds.
- Path-containment and MIME-mismatch security tests.
- Golden rendering tests for image, video, audio, and fallback cards.
- Playback state tests for pause, seek, end, close, session switch, and failure.
- Bounded-cache and cancellation tests under repeated generation.
- PTY scenarios for each supported graphics protocol and no-graphics fallback.
- Windows, macOS, Linux, minimal mode, and npm-packaged binary smoke tests.

## Completion criteria

New Chutes media outputs reach the TUI without text scraping; images preview
inline where supported; video starts from a poster without unbounded frame
retention; audio has local TUI controls; every unsupported environment retains
a reliable `[Open]` fallback; and generated content never leaves the local
machine except for the Chutes generation request explicitly initiated by the
user.
