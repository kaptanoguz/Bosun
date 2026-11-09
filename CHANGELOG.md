# Changelog

The format is inspired by [Keep a Changelog](https://keepachangelog.com/); versions follow [SemVer](https://semver.org/).

## [Unreleased]

### Changed
- The top status bar is gone. Each page starts with its own title; the overall state
  ("Needs attention", "All good") and the time of the last reading sit at the bottom of the side bar.
- Overview cards stay in a tidy grid at every window width (three per row on narrow windows
  instead of leaving holes). The disk card shows free space against the disk size.
- History shows one line per cleanup instead of separate "suggested", "approved" and "freed"
  lines. Plans that were never applied are dimmed; administrator actions show their name
  ("Shrink system logs") instead of an internal id.
- Findings for large system logs, the APT cache and unused Flatpak runtimes no longer tell you
  to run a terminal command; Bosun can do it from the Cleanup tab.
- Language and theme pickers no longer use a filled brass button; brass is kept for primary
  actions.
- English text says "1 item" instead of "1 items".

### Internal
- The sample data for the browser preview is loaded only in the browser and is no longer part
  of the desktop app's main bundle.
- Connection, readings, scan and chat state moved out of `App.tsx` into a `useDaemon` hook.

## [0.5.0] — 2026-11-09

### Added
- **Busiest apps:** processes are grouped by program and the Overview lists the apps using the
  most CPU and memory right now. The chat assistant sees the same list.
- **Busy CPU finding:** when the CPU stays above 90% for a minute, Bosun says which app is behind
  it ("CPU is busy: bun uses 61% of it"). The memory finding now names the largest apps too.
- **Memory growth finding:** an app whose memory keeps growing steadily for 45 minutes or more
  (at least 100 MB per hour) is reported, with the rate and how much it uses now.
- **Disk activity:** read and write speed of physical disks.
- **Startup apps:** a new tab lists the apps that start at login and turns them off or back on.
  Nothing is deleted (a copy with `Hidden=true` goes to `~/.config/autostart`); parts of the
  desktop can't be turned off. Every change is in History.
- **Containers:** running Docker / Podman containers with their CPU and memory, refreshed every
  30 seconds. Read-only.
- New thresholds: `cpu_busy`, `cpu_sustain_secs`, `leak_mb_per_hour`, `leak_minutes`.

### Changed
- Calmer, denser look: compact metric cards with a sparkline each instead of large dials,
  findings as banners at the top, tables for apps and containers, the system font instead of
  Barlow, neutral surfaces with brass kept as the only accent.

## [0.4.2] — 2026-11-09

### Fixed
- Cleaned-up items could reappear right after a cleanup: a rescan requested while an older scan
  was still running returned that older scan's result. A rescan now always waits for a scan that
  started after the request.
- Files Bosun can't remove with your own rights (a folder that belongs to another user, usually
  because a program such as waydroid-script ran with `sudo`) no longer show up as cleanable and
  then "fail" on every cleanup. They are counted separately, and the category shows the folder
  and the command that gives it back to you.
  This also covers a cache folder that is itself read-only: moving it to the trash needs write
  access to the folder, not only to the one above it.

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
