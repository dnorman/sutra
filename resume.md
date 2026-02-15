# Sutra — Resume for Fresh Agent

## What is this?

Sutra is a macOS status dashboard for dev.sh environments. It watches `~/.dev-runner/` and displays each environment's units in a GUI (iced) or TUI (ratatui), with system sounds, speech, and macOS notifications on state transitions.

The crate was renamed from `viser` to `sutra`. The repo directory is still `/Users/daniel/code/viser` but the crate name, all internal references, and the GitHub remote are `sutra` (`git@github.com:dnorman/sutra.git`).

## Current state (2026-02-15)

**Compiles clean** (`cargo check` passes, 1 minor warning: unused WatchEvent field payload — the handler ignores the event ID and does a full reload).

### Architecture

```
src/
  main.rs           # CLI: `sutra` defaults to `sutra mon` (GUI), `sutra mon --tui` for TUI
                    # GUI mode backgrounds by default (re-execs with --foreground)
                    # `sutra mon --foreground` keeps attached to terminal
  lib.rs            # Feature-gated modules
  model.rs          # Environment, UnitStatus, State enum, file parsing
  watcher.rs        # FSEvents watcher on ~/.dev-runner/, emits WatchEvent
  notifications.rs  # Notifier: sound (rodio), speech (tts AppKit), macOS notifications
                    # Transition detection, global + per-unit mute/notification toggles
                    # Bundle ID: io.github.dnorman.sutra
  gui.rs            # iced GUI: cards, SVG toolbar icons, per-unit toggles, hover, terminate
                    # Dock icon via objc2 NSApplication.setApplicationIconImage
                    # Async watcher bridge (futures::channel::mpsc, no longer blocks main thread)
  tui.rs            # ratatui TUI: mouse support, unit selection cursor, auto-scroll
                    # Keys: j/k select, m/n global toggles, M/N per-unit, o open, x terminate, q quit
assets/
  icon.png              # 256x256 app icon (dark purple bg, green eye)
  icon-transparent.png  # 256x256 transparent version (used in README)
```

### Terminology

- **Environment** = a dev-runner instance (identified by DIR, meta file `<hex-hash>`)
- **Unit** = a subprocess within an environment (status file `<hash>.<unit>.status`)
- Fully consistent in all .rs files and all .md files

### File convention — `~/.dev-runner/`

Full spec in `STATE_SPEC.md`. Summary:

```
<hex-hash>                          # Meta file: KEY=VALUE lines (DIR, PID, STARTED, *_PORT)
<hex-hash>.<unit_name>.status       # Status: "state[: detail]"
.<hex-hash>.<unit_name>.status      # Old convention (leading dot) — also supported
```

States: `starting`, `building`, `running`, `ready`, `failed`, `stopped`, or any arbitrary string (→ `State::Other`).

### State indicators (visual progression)

```
○  None/Stopped    (empty circle — inactive)
◌  Starting        (dotted circle — forming)
◑  Building        (half-filled — in progress)
●  Running/Ready   (filled circle — active)
✗  Failed          (X mark — failure)
◆  Other           (diamond — unknown)
```

### Features implemented

- Real-time FS watching (async bridge, non-blocking) + 2s periodic refresh
- Color-coded unit states (green/yellow/red/gray) with theme-aware palettes (dark=Dracula, light=GitHub)
- Light mode (default) + dark mode toggle (sun/moon SVG icon)
- Lucide SVG icons in toolbar: volume-2/x (mute), bell/bell-off (notifications), sun/moon (theme)
- Per-unit SVG icons: volume + bell toggles left of each unit row
- Hover highlighting on unit rows (mouse_area enter/exit, subtle background)
- Fixed-width table columns via pixel-width containers (name, port, state columns align properly)
- Sound on state transitions: Submarine (building/starting), Ping (ready/running), Basso (failed)
- Speech says actual state name (e.g., "server running", NOT hardcoded "server ready")
- macOS notification center banners (bundle ID: `io.github.dnorman.sutra`)
- Global mute + per-unit mute (independent of notifications)
- Global notifications toggle + per-unit notifications toggle
- Generic `*_PORT` parsing from meta files (any NAME_PORT key)
- Ports displayed in unit rows with fixed-width alignment
- `↗` open-browser button for units with ports (`open http://localhost:{port}`)
- Terminate environment button (⏹ in GUI header, `x` in TUI) — sends SIGHUP to env PID
- Subcommand structure: `sutra mon [--tui] [--foreground]` with `mon` as default
- GUI backgrounds by default (daemonize via re-exec), `--foreground` to keep attached
- macOS dock icon via objc2 NSApplication.setApplicationIconImage
- Window icon via iced window settings
- TUI: mouse click to select units, scroll wheel, auto-scroll to follow cursor
- TUI: `o` opens browser for selected unit's port, `x` terminates selected env

### Dependencies

```toml
notify = "7"                          # filesystem watcher
dirs = "6"                            # home dir
nix = "0.29"                          # PID liveness + SIGHUP
clap = "4"                            # CLI
rodio = "0.21"                        # system sounds (.aiff playback)
tts = "0.26"                          # speech (AppKit backend)
mac-notification-sys = "0.6"          # notification center
ratatui = "0.29" + crossterm = "0.28" # TUI (optional), mouse capture enabled
iced = "0.13" (tokio, svg, image)     # GUI (optional)
objc2 = "0.5"                         # macOS dock icon
objc2-foundation = "0.2"              # NSData
objc2-app-kit = "0.2"                 # NSApplication, NSImage
```

### CI / Publishing

- `.github/workflows/ci.yml` — test + fmt + clippy, matrix build (macOS x86_64 + aarch64)
- `.github/workflows/publish.yml` — auto-publish to crates.io on Cargo.toml version bump, creates git tag
- `.github/workflows/release.yml` — builds stripped binaries, attaches to GitHub release
- `LICENSE-MIT` + `LICENSE-APACHE` — dual licensed
- Remote: `git@github.com:dnorman/sutra.git` (on `master` branch, main branch is `main`)

### Test data

- `create-test-envs.sh` — creates 15 fake environments (53 units) in `~/.dev-runner/` for scroll testing
  - Run: `bash create-test-envs.sh`
  - Clean: `bash create-test-envs.sh --clean`
  - Uses hex IDs `a0a0a0a0a0a00001` through `a0a0a0a0a0a0000f`, PIDs 99999-99985 (dead)
- `df79fed95eebc05d` — real properlydone-platform entry (5 units: metro, mobile, server, vite, wasm)

## Known issues / Active bugs

### CRITICAL: GUI crashes with "sutra quit unexpectedly"

**Symptom**: SIGABRT crash, `panic_cannot_unwind` inside CoreFoundation run loop → iced/winit NSApplication::run. Happens intermittently, more frequently with many environments loaded.

**Crash stack**: Thread 0 aborts because a Rust panic occurs inside an Objective-C callback boundary (CFRunLoop block) where unwinding is not allowed. The panic originates somewhere in iced's event handling code path.

**What we know**:
- Crash reports in `~/Library/Logs/DiagnosticReports/sutra-*.ips`
- Was initially triggered by `set_dock_icon()` being called inside `update()` — moved to before `iced::application().run()` but crashes persist
- May be related to `cargo watch` restarting the process while NSApplication is running (user was running cargo watch)
- The `set_dock_icon()` function uses objc2 to call `NSApplication.setApplicationIconImage` — this is called before iced starts but could still be problematic if iced reinitializes NSApplication
- Could also be a panic in view/update code that gets caught at the ObjC boundary

**Next step**: Add diagnostic logging (eprintln or file-based) in `update()`, `view()`, and `set_dock_icon()` to identify exactly which code path is panicking. Consider wrapping `set_dock_icon()` in `std::panic::catch_unwind()`. Try running with `RUST_BACKTRACE=1 cargo run -- mon --foreground` to get a backtrace before the crash.

### MEDIUM: Remaining code quality issues from review

These were identified by a code review agent and are still unaddressed:

1. `state_variant_eq` silently ignores `Other("compiling")` → `Other("linking")` transitions (notifications.rs:211)
2. `pid as i32` cast can overflow (model.rs:150) — safe in practice on macOS
3. `registry_dir()` panics with `expect` on missing home dir (model.rs:232) — should return Result
4. `UnitStatus::parse` always returns `Some` — Option return type is misleading (model.rs:81)
5. `is_unit_muted` / `is_unit_notifications_off` allocate 2 Strings per call for HashSet lookup (notifications.rs:179)
6. Duplicated App struct/init between GUI and TUI

### LOW: UX improvements identified

1. Light-mode yellow (`#b08800`) fails WCAG contrast on white
2. Empty state shows "No environments found." with no guidance
3. No macOS keyboard shortcuts (Cmd+Q, etc.)
4. Speech is on by default with no independent toggle — can be surprising/annoying
5. Should follow system dark/light appearance
6. Rapid-fire notifications not batched (5 rebuilds = 15 seconds of serial audio)

## Commands

```bash
cargo check                                        # verify all features
cargo run                                          # GUI (backgrounds by default)
cargo run -- mon --foreground                      # GUI, attached to terminal (for debugging)
cargo run -- mon --tui                             # TUI
cargo run --no-default-features --features tui     # TUI-only binary
RUST_BACKTRACE=1 cargo run -- mon --foreground     # debug crashes
bash create-test-envs.sh                           # create 15 fake environments
bash create-test-envs.sh --clean                   # remove fake environments
```

## Related files

- `STATE_SPEC.md` — full spec for the `~/.dev-runner/` file format
- `README.md` — user-facing docs with icon, install, usage
- `create-test-envs.sh` — test data generator
- Real dev.sh: `~/vt/properlydone/code/properlydone-platform/dev.sh`
