<p align="center">
  <img src="assets/icon-transparent.png" width="128" alt="sutra">
</p>

# sutra

[![CI](https://github.com/dnorman/sutra/actions/workflows/ci.yml/badge.svg)](https://github.com/dnorman/sutra/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/sutra.svg)](https://crates.io/crates/sutra)

A status dashboard for [dev.sh](https://github.com/dnorman/dev.sh) environments. Sutra watches `~/.dev-runner/` and stitches each environment's meta + per-unit state into a single glanceable view -- GUI (iced) or TUI (ratatui) -- with macOS notifications, system sounds, and spoken state changes when a unit transitions (all mutable globally or per-unit).

Currently macOS-only, but the architecture is platform-agnostic.

## Install

```sh
cargo install sutra
```

Or from source:

```sh
git clone https://github.com/dnorman/sutra && cd sutra
cargo install --path .
```

## Usage

```sh
sutra              # launch GUI monitor (default)
sutra mon          # same, explicit
sutra mon --tui    # launch terminal UI
```

### GUI controls

- Click the speaker icon to toggle global mute
- Click per-unit speaker/bell icons to mute sound or notifications individually
- Click the sun/moon icon to toggle light/dark mode

### TUI controls

| Key | Action |
|-----|--------|
| `q` | Quit |
| `r` | Refresh |
| `j`/`k` | Scroll |
| `m` | Toggle mute |

## How it works

Sutra monitors `~/.dev-runner/` for filesystem changes and re-reads the registry on every event.

### Registry format

**Meta files** -- `~/.dev-runner/<hex-hash>`

Plain `KEY=VALUE` lines describing an environment:

```
DIR=/Users/you/myproject
PID=12345
STARTED=1700000000
SERVER_PORT=3000
VITE_PORT=5173
```

Any `*_PORT` key is picked up automatically and displayed next to the matching unit.

**Status files** -- `~/.dev-runner/<hex-hash>.<unit>.status`

Single-line files: `state[: detail]`

```
building: Compiling Rust bindings
ready
failed: exit code 1
```

Well-known states (`starting`, `building`, `running`, `ready`, `failed`, `stopped`) get distinct colors, indicators, and sounds. Any other string is accepted and rendered with a neutral fallback.

### State transitions

When a unit changes state, sutra can:

- Play a system sound (Submarine for building, Ping for ready, Basso for failed)
- Speak the transition ("server ready", "vite building")
- Send a macOS notification

All of this is suppressed on first load (snapshot only) and respects mute settings.

### Voice

Sutra uses the macOS system default voice (same as `say`) via the AppKit speech backend. To change the voice, update it in **System Settings > Accessibility > Spoken Content > System Voice**.

## Features

- `gui` -- iced-based native window (default)
- `tui` -- ratatui terminal interface (default)

Build with only one:

```sh
cargo build --no-default-features --features tui
```

## License

MIT OR Apache-2.0
