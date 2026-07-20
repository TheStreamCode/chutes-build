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

### Security

- Restricted credential-bearing media invocation to Chutes HTTPS hosts,
  disabled credential-bearing redirects, and bounded generated-media and input
  asset sizes.
- Restricted browser and media output paths to canonical workspace locations,
  prevented silent file overwrite, and redacted password values from browser
  snapshots.
- Added full-history secret scanning, Rust dependency/source policy checks,
  native archive checksums, and assembled-package smoke testing to CI.
