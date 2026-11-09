# Changelog

The format is inspired by [Keep a Changelog](https://keepachangelog.com/); versions follow [SemVer](https://semver.org/).

## [Unreleased]

## [0.4.1] — 2026-11-09

### Changed
- New logo: a bridge gauge. A solid brass arc for the reading, a slate track and a
  counterweighted needle; a simplified variant without ticks and with thicker strokes for
  32 px and below.
- README rewritten in English with screenshots; the Turkish version lives in `README.tr.md`.
- Browser preview sample data is now fully English.

## [0.4.0] — 2026-11-09

### Added
- **Streaming chat:** replies appear as they are written, and you can see which reading the
  assistant is looking at (all Anthropic and OpenAI-compatible providers).
- Network throughput, battery (state and power), GPU (NVIDIA and AMD) and load average on the
  Overview; a finding when the battery drops below 15%.
- **AppImage:** a single file. "Start service" points the service at the same file with
  `--daemon`. Administrator actions need the helper from the .deb or `install.sh --helper`.
- Tagged release workflow: .deb + AppImage, SHA256SUMS, optional GPG signature, release notes
  from this changelog.

## [0.3.1] — 2026-11-09

### Added
- **Mute** per finding: a muted finding sends no notifications and does not affect the overall
  status (`muted_findings`).
- Search (category name, description, path) and a type filter in the cleanup list.
- Steam shader caches.
- Docker/Podman leftovers: only untagged (dangling) images and build cache, with fixed commands
  and your own user rights.

### Fixed
- Flatpak: runtimes removed but whose folder remains (no "active" link), as well as GL, VAAPI,
  codec and theme extensions, are no longer listed as "unused".

## [0.3.0] — 2026-11-09

### Added
- Scan progress (category by category) and a **Stop scan** button.
- A "list truncated" notice for categories that hit the item limit.
- Developer caches: pip, uv, Poetry, conda, Go, Yarn, Gradle.
- **Look inside** preview for old program folders (largest subfolders, file count, last change).
- Settings: switches for sharing file paths, notifications and chat history; theme
  (System/Dark/Light); **Copy diagnostics** (API keys are redacted).
- Chat history is saved to disk (`~/.local/state/bosun/chats.json`, 0600) and comes back after
  the service restarts.
- The last known readings are shown while the service is down.
- `stale_days` and `large_file_mb` thresholds in the config.
- GitHub Actions CI (fmt, clippy, tests, frontend, .deb) and socket integration tests against
  the real service.

### Changed
- A single journald target: 500 MB for both the cleanup and the persistent limit.
- The audit log is rotated once it exceeds 2 MB (about 4 MB at most).
- Tool output sent to the assistant is capped at 12,000 characters.
- Administrator action results show what the system actually reported.

### Fixed
- Flatpak: pinned runtimes, app extensions and GTK themes are no longer counted as "unused";
  things Flatpak would not remove are not listed.

## [0.2.0] — 2026-11-09

### Added
- Choice of chat provider: Anthropic Claude, xAI Grok, Google Gemini, OpenAI and
  OpenAI-compatible custom servers (Ollama, LM Studio). Keys are entered in Settings and stored
  per provider.
- "Select all / Clear selection" on pick-one-by-one lists.
- A warning that explains why the helper can't be used (missing / not owned by root / writable
  by others) and shows the command that fixes it.

### Fixed
- A helper installed with the `.deb` showed as "not installed" and **Continue** stayed disabled
  for administrator actions. `ProtectSystem`/`PrivateTmp` in the systemd user unit created a
  separate user namespace in which root-owned files appeared owned by `nobody`. Those options
  were removed, and helper status is now read from the app process that runs pkexec.

## [0.1.0] — 2026-11-09

### Added
- First release: monitoring service (`bosund`), Tauri app, approved cleanup plans, audit log.
- Phase 2: administrator actions (`bosun-helper` + polkit), apply/undo for settings advice,
  Flatpak, broken links and shortcuts, leftover packages, old program folders, tray icon,
  English/Turkish interface, a single `.deb` package.
