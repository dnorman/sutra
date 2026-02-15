<p align="center">
  <img src="assets/icon-transparent.png" width="128" alt="sutra">
</p>

# sutra

[![CI](https://github.com/dnorman/sutra/actions/workflows/ci.yml/badge.svg)](https://github.com/dnorman/sutra/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/sutra.svg)](https://crates.io/crates/sutra)

A macOS status dashboard for dev environments. Monitors a well-known state folder for environment meta and per-unit status files, rendering everything in a native GUI (iced) or TUI (ratatui) with sounds, speech, and notifications on state transitions.

## Install

```sh
cargo install sutra
```

## Usage

```sh
sutra                       # launch GUI (backgrounds by default)
sutra mon --foreground      # GUI, attached to terminal
sutra mon --tui             # terminal UI
```

Both interfaces support per-unit and global toggles for sound and notification muting, light/dark mode, environment termination, and opening browser ports.

## Features

- `gui` -- iced-based native window (default)
- `tui` -- ratatui terminal interface (default)

Build with only one:

```sh
cargo build --no-default-features --features tui
```

## License

MIT OR Apache-2.0
