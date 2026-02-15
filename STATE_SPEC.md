# `.dev-runner/` State Specification

This document defines the file format and conventions for `~/.dev-runner/`, the well-known location shared between **dev.sh** (the environment runner) and **sutra** (the status dashboard).

## Overview

`~/.dev-runner/` is a flat directory containing two kinds of files:

1. **Meta files** — one per environment, describing the running instance
2. **Status files** — one per unit within an environment, describing its current state

No locking, coordination, or daemon is required. Writers create/update files atomically; readers scan the directory and parse what they find.

## Directory

```
~/.dev-runner/
```

Created automatically by either dev.sh or sutra if it does not exist.

## Meta Files

### Path

```
~/.dev-runner/<id>
```

`<id>` is a hex string (lowercase `[0-9a-f]+`) that uniquely identifies an environment instance. Typically a hash derived from the project directory path.

A valid meta filename:
- Contains only ASCII hex digits
- Does not start with `.`
- Does not contain `.`

### Format

Plain text, one `KEY=VALUE` pair per line. No quoting, no escaping, no comments. Unrecognized keys are ignored.

#### Required keys

| Key       | Type   | Description |
|-----------|--------|-------------|
| `DIR`     | path   | Absolute path to the project directory |
| `PID`     | u32    | Process ID of the environment's main process |

#### Optional keys

| Key       | Type   | Description |
|-----------|--------|-------------|
| `STARTED` | u64    | Unix epoch timestamp (seconds) when the environment was started |
| `*_PORT`  | u16    | Any key ending in `_PORT` declares a port. The prefix (lowercased, with `_PORT` stripped) is matched against unit names. |

#### Port matching

A key like `SERVER_PORT=3000` maps to unit name `server` (lowercase prefix). Multiple port keys are supported:

```
SERVER_PORT=3000
VITE_PORT=5173
METRO_PORT=8081
```

#### Example

```
DIR=/Users/you/code/myproject
PID=48291
STARTED=1700000000
SERVER_PORT=3000
VITE_PORT=5173
```

### Lifecycle

- **Created** by dev.sh when an environment starts
- **Deleted** by dev.sh when an environment stops (along with all associated status files)
- **Read** by sutra on each refresh or filesystem event
- **Liveness**: sutra checks `kill(PID, 0)` to determine if the environment is still alive. A meta file with a dead PID is shown as inactive but not automatically removed.

## Status Files

### Path (two conventions)

```
~/.dev-runner/<id>.<unit_name>.status       # current convention
~/.dev-runner/.<id>.<unit_name>.status      # legacy convention (leading dot)
```

Both are recognized by sutra. New writers should use the current convention (no leading dot).

`<unit_name>` is a lowercase identifier for the unit (e.g., `server`, `vite`, `wasm`, `metro`, `mobile`). It must not contain `.` characters.

### Format

Single line of text:

```
<state>[: <detail>]
```

The state is a lowercase keyword. The optional detail (after `": "`) is freeform text providing additional context.

### Well-known states

| State      | Meaning | Indicator | Color  | Sound     |
|------------|---------|-----------|--------|-----------|
| `starting` | Unit is initializing | `…` | yellow | Submarine |
| `building` | Unit is compiling/bundling | ◔ | yellow | Submarine |
| `running`  | Unit process is active | `●` | green  | Ping      |
| `ready`    | Unit is serving/accepting connections | `●` | green  | Ping      |
| `failed`   | Unit crashed or build failed | `✗` | red    | Basso     |
| `stopped`  | Unit was intentionally stopped | `○` | gray   | —         |

Any other string is accepted as `Other` and rendered with a neutral gray diamond (`◆`). Empty or missing status files are treated as `None` (empty circle `○`).

### Detail examples

```
building: Compiling Rust bindings
building: wasm-pack
failed: exit code 1
failed: iOS build failed
building: xcode build running
```

### Writing status files

Writers should update status files **atomically** — write to the file directly (single-line content means partial writes are unlikely, but `echo "state" > file` is sufficient for this use case).

Status files should be written **before** any corresponding notification or speech call, so that dashboard UIs can react to the transition promptly.

### Cleanup

When an environment stops, **all** of its status files must be removed:

```bash
rm -f "$REGISTRY_DIR/$KEY".*.status
rm -f "$REGISTRY_DIR/.$KEY".*.status   # legacy cleanup
```

This should happen in stop handlers, cleanup traps, and unregister functions.

## State Transitions & Notifications

Sutra detects state transitions by comparing current state against the previously observed state for each `(environment_id, unit_name)` pair.

### First load

On startup, sutra snapshots all current states **without** firing any notifications. This prevents a storm of sounds when the dashboard launches into an already-running environment.

### Transition rules

A notification (sound + speech + macOS banner) fires when:
- The state variant changes (e.g., `building` → `ready`)
- A new unit appears for the first time (after first load)

A notification does **not** fire when:
- The state variant is the same (even if detail text changes)
- Transitioning between `None` and `Other` states

### Sound mapping

| Transition target | System sound |
|-------------------|-------------|
| `starting`, `building` | `/System/Library/Sounds/Submarine.aiff` |
| `running`, `ready` | `/System/Library/Sounds/Ping.aiff` |
| `failed` | `/System/Library/Sounds/Basso.aiff` |
| `stopped`, `None`, `Other` | (silent) |

### Speech

After the system sound, sutra speaks `"<unit_name> <state>"` using the macOS system voice (AppKit NSSpeechSynthesizer, same voice as `say`).

### macOS notifications

A banner notification is sent via Notification Center with:
- **Title**: `sutra — <unit_name>`
- **Body**: the state string (e.g., `ready`, `failed: exit code 1`)

### Muting

Notifications are independently controllable:
- **Global sound mute**: silences all sounds and speech
- **Per-unit sound mute**: silences sounds/speech for a specific unit
- **Global notifications off**: suppresses macOS banners
- **Per-unit notifications off**: suppresses banners for a specific unit

Sound mute and notification suppression are independent — you can mute audio while still receiving banners, or vice versa.

## Example Layout

```
~/.dev-runner/
  df79fed95eebc05d                          # meta: DIR, PID, ports
  df79fed95eebc05d.server.status            # "running"
  df79fed95eebc05d.vite.status              # "ready"
  df79fed95eebc05d.wasm.status              # "building: wasm-pack"
  df79fed95eebc05d.metro.status             # "running"
  df79fed95eebc05d.mobile.status            # "building: xcode build running"
```

## Compatibility Notes

- sutra recognizes both dotted (`.hash.unit.status`) and non-dotted (`hash.unit.status`) conventions
- Meta filenames must be pure hex — any file containing `.` is treated as a status file or ignored
- Keys in meta files are case-sensitive (`DIR`, not `dir`)
- State strings in status files are case-sensitive (`ready`, not `Ready`)
- The `~/.dev-runner/` path is not configurable (hardcoded in both dev.sh and sutra)
