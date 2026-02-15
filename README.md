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
sutra                       # launch GUI (backgrounds by default)
sutra mon                   # same, explicit
sutra mon --foreground      # GUI, attached to terminal (for debugging)
sutra mon --tui             # launch terminal UI
```

### GUI controls

- **Speaker icon** -- toggle global sound mute
- **Bell icon** -- toggle global notifications
- **Sun/moon icon** -- toggle light/dark mode
- **Per-unit speaker/bell icons** -- mute sound or notifications per unit
- **Red square** on environment header -- terminate environment (SIGTERM)
- **↗** next to units with ports -- open `localhost:{port}` in browser
- **Cmd+Q** -- quit

### TUI controls

| Key | Action |
|-----|--------|
| `q` | Quit |
| `r` | Force refresh |
| `j` / `k` / `↑` / `↓` | Select previous/next unit |
| `m` | Toggle global sound mute |
| `n` | Toggle global notifications |
| `M` | Toggle sound mute for selected unit |
| `N` | Toggle notifications for selected unit |
| `o` | Open browser for selected unit's port |
| `x` | Terminate selected unit's environment (SIGTERM) |

Mouse: click to select a unit, scroll wheel to move selection.

### State indicators

```
○  Stopped/None     (empty circle)
◌  Starting         (dotted circle)
◑  Building         (half-filled circle)
●  Running/Ready    (filled circle)
✗  Failed           (X mark)
◆  Other            (diamond)
```

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
- Speak the transition ("server ready, vite building" -- batched into one utterance)
- Send a macOS notification

All of this is suppressed on first load (snapshot only) and respects mute settings. When multiple units change simultaneously, audio is batched: one sound (highest priority) and one combined speech utterance.

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
