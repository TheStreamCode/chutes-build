# Changelog

All notable changes to Chutes Build will be documented in this file. The format
is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Chutes-native inference, model discovery, automatic routing, and fallback
  handling.
- Adaptive reasoning controls backed by a centralized model capability
  registry.
- Advisor and parallel subagent orchestration.
- Built-in Context7, official Chutes research, web search, browser automation,
  and project memory.
- Chutes usage indicators for rolling four-hour and monthly limits.
- Typed image, video, and audio artifacts with bounded, opt-in terminal
  previews and native-player fallbacks.
- Privacy-first defaults with telemetry, remote trace upload, and automatic
  update checks disabled.
- Cross-platform npm launcher and native package pipeline.

### Changed

- Rebranded the public product, binary, user data directory, and themes as
  Chutes Build.
- Cached FFmpeg and package-manager discovery for the lifetime of the process
  so idle TUI rendering does not repeatedly probe external commands.
- Bounded account quota fallback concurrency and cached model-capability
  discovery to reduce unnecessary API work.

### Fixed

- Interactive login: the welcome screen and `/login` had no working path to
  the OAuth method the rebrand advertised, and no way to enter an API key
  from inside the running app at all (only the separate `chutes-build login`
  CLI subcommand worked, before ever starting the TUI). Added "Sign in with
  Chutes" OAuth end-to-end (issuer, client, scopes, loopback callback) and an
  in-TUI API key entry reachable from the welcome screen (`k`), `/login`, and
  `/apikey`, and fixed two stale-state bugs (`auth_show_raw_url`,
  `welcome_prompt_focused`) that silently swallowed keyboard input on the
  auth screen.
- `get_chutes_usage` and all media tools 401'd by default: the account/media
  HTTP client sent the API key without the `Bearer` prefix `api.chutes.ai`
  requires.
- The macOS native build failed to compile (a stray `cfg` left an
  `std::process` import unreachable in a macOS-only module).
- The secret sanitizer that filters Sentry/Mixpanel/log output did not
  recognize the `cpk_` Chutes API key prefix, so real keys were not redacted
  from those sinks.
- The browser automation tool left a dead session in place after any
  connection-level failure (closed socket, crashed browser), making it
  unusable for the rest of the session until an explicit `close`.
- `generate_media` only validated the top-level shape of `params` against a
  cord's schema, so a payload with the right outer wrapper (e.g. `args`) but
  wrong fields nested inside it passed local validation and round-tripped to
  Chutes for a generic "Invalid input parameters" error instead of a precise
  local one.
- The terminal window/tab title showed "grok" instead of "chutes-build".

### Security

- Restricted credential-bearing media invocation to Chutes HTTPS hosts,
  disabled credential-bearing redirects, and bounded generated-media and input
  asset sizes.
- Restricted browser and media output paths to canonical workspace locations,
  prevented silent file overwrite, and redacted password values from browser
  snapshots.
- Added full-history secret scanning, Rust dependency/source policy checks,
  native archive checksums, and assembled-package smoke testing to CI.
