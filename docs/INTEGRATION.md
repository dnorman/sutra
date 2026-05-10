# Integrating sutra with your dev script

> **Hi, agent.** This doc is written for you. A user has pointed you
> here and asked you to make their project's dev script sutra-compatible.
>
> Your job:
> 1. Find the project's existing dev runner (`dev.sh`, `dev.ts`,
>    `Justfile`, `Makefile`, `package.json` scripts, `bin/dev`, etc.).
>    If none exists, create one.
> 2. Make it satisfy the contract below, applying the patterns in
>    "Recommended pattern" that match the project's stack.
> 3. Verify with the "Checklist" section before declaring done.
>
> Don't rewrite a working dev runner. Add the registry/status hooks
> alongside whatever it already does.

Sutra reads `~/.dev-runner/`. Any dev runner that writes the right files
gets a live status row in the dashboard — no daemon, no IPC, no SDK.
Just files.

For exact file format and edge cases, see [STATE_SPEC.md](../STATE_SPEC.md).

## The contract

Two file shapes live in `~/.dev-runner/`:

**1. A meta file** named after a hex hash of your project path. One per
running environment.

```
~/.dev-runner/<id>
```

Plain `KEY=VALUE`, one per line. Required: `DIR`, `PID`. Optional:
`STARTED`, any `*_PORT`.

**2. Status files**, one per subprocess (server, build watcher, dev
server, …):

```
~/.dev-runner/<id>.<unit_name>.status
```

Single line: `<state>` or `<state>: <detail>`. The canonical states
are `starting`, `building`, `running`, `ready`, `failed`, `stopped`.
Stick to these — anything else is accepted but won't get the right
semantic treatment downstream.

When the environment exits, delete both. That's the whole protocol.

## Minimum recipe

The smallest script that lights up sutra:

```bash
#!/bin/bash
set -e

REG="$HOME/.dev-runner"; mkdir -p "$REG"
ID=$(echo -n "$PWD" | shasum -a 256 | cut -c1-16)
META="$REG/$ID"

trap 'rm -f "$META" "$REG/$ID".*.status' EXIT INT TERM

cat > "$META" <<EOF
DIR=$PWD
PID=$$
STARTED=$(date +%s)
SERVER_PORT=3000
EOF

echo "starting" > "$REG/$ID.server.status"
# ... start your server ...
echo "ready"    > "$REG/$ID.server.status"
wait
```

Run this and sutra immediately shows a card with a green dot next to
"server" and a clickable `:3000` open-in-browser link.

## Recommended pattern

For a real dev runner — multiple subprocesses, restart support, sticky
ports — the snippets below mirror the structure of a working production
script. Adapt to taste.

### 1. Set up registry handles

```bash
# Central registry shared across all sutra-aware projects
REGISTRY_DIR="$HOME/.dev-runner"
mkdir -p "$REGISTRY_DIR"

# Stable per-project id derived from the project path
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REGISTRY_KEY=$(echo -n "$SCRIPT_DIR" | shasum -a 256 | cut -c1-16)
REGISTRY_FILE="$REGISTRY_DIR/$REGISTRY_KEY"
```

A 16-char hex prefix is plenty; sutra accepts any lowercase hex string.
Hashing the path means the same project gets the same id across runs,
but two checkouts of the same project get different ids.

### 2. Write the meta file when you start

```bash
register_instance() {
    cat > "$REGISTRY_FILE" <<EOF
DIR=$SCRIPT_DIR
PID=$1
SERVER_PORT=$SERVER_PORT
FRONTEND_PORT=$FRONTEND_PORT
STARTED=$(date +%s)
EOF
}
```

- `DIR` — sutra shows the basename as the friendly project name.
- `PID` — sutra polls with `kill -0` to mark the env alive/dead.
- `*_PORT` — declares a port and matches by lowercase prefix to the
  unit name. `SERVER_PORT` → `server`, so the row gets a `↗`
  open-in-browser affordance.

### 3. Write status updates from each subprocess

```bash
# Convention: "<state>" or "<state>: <detail>"
update_status() {
    local name="$1" status="$2"
    echo "$status" > "$REGISTRY_DIR/$REGISTRY_KEY.$name.status"
}

# Export so subshells, cargo-watch, npm scripts can call it
export REGISTRY_DIR REGISTRY_KEY
export -f update_status
```

There are two ways to wire this up. Use whichever fits the subprocess.

#### a. Wrap the subprocess (when it's yours)

For code you control or a wrapper script that already brackets the
process, write states directly:

```bash
update_status server "building: cargo"
if cargo build --release; then
    update_status server "running"
    cargo run --release
else
    update_status server "failed: build error"
fi
```

For a watcher (cargo-watch, esbuild …) put `update_status` calls in
the watcher's pre/post-build hooks so the dot flips yellow ↔ green on
every rebuild.

#### b. Poll readiness from the side (when it isn't)

For third-party servers like uvicorn, Vite, Metro, or anything else
you don't want to fork or pipe-monitor, run a small sidecar loop that
flips status to `ready` once a probe URL responds:

```bash
update_status server "starting"
update_status vite   "starting"

# Server (uvicorn, gunicorn, …)
( cd "$SCRIPT_DIR/server" && exec ./run-server.sh --port "$SERVER_PORT" ) &

# UI (Vite, Next, …)
( cd "$SCRIPT_DIR/ui" && exec npm run dev -- --port "$VITE_PORT" ) &

# Readiness probe — flip to "ready" when each URL responds
(
    server_ready=false
    vite_ready=false
    for ((i=0; i<60; i++)); do
        if [ "$server_ready" = false ] && \
           curl -sf "http://127.0.0.1:$SERVER_PORT/health" >/dev/null 2>&1; then
            update_status server "ready"
            server_ready=true
        fi
        if [ "$vite_ready" = false ] && \
           curl -sf "http://localhost:$VITE_PORT/" >/dev/null 2>&1; then
            update_status vite "ready"
            vite_ready=true
        fi
        [ "$server_ready" = true ] && [ "$vite_ready" = true ] && break
        sleep 1
    done
    [ "$server_ready" = false ] && update_status server "failed: timeout"
    [ "$vite_ready"   = false ] && update_status vite   "failed: timeout"
) &
```

The sidecar is a single backgrounded subshell that lives only until
both probes succeed (or the 60s budget expires). It doesn't need to
keep running — sutra will continue to show the last-written state
until you `clear_all_status` on shutdown.

#### Self-clearing transient units

Short-lived units (`build`, `seed`, `migrate`) shouldn't linger as
stale `ready` rows after they finish. Have them remove their own file
a beat after completing:

```bash
update_status build "ready"
( sleep 2; rm -f "$REGISTRY_DIR/$REGISTRY_KEY.build.status" ) &
```

The 2-second window is enough for the user to see the success state
and for sutra to fire its transition notifications; after that the
row disappears so it doesn't sit around as a stale `ready`.

### 4. Always clean up

Status files outlive the script if you forget to remove them. Put the
cleanup in a trap **and** in your `--stop` handler:

```bash
clear_all_status() {
    rm -f "$REGISTRY_DIR/$REGISTRY_KEY".*.status
}

unregister_instance() {
    clear_all_status
    rm -f "$REGISTRY_FILE"
}

trap unregister_instance EXIT INT TERM HUP
```

It's also worth calling `clear_all_status` at the **start** of a fresh
run, not just on exit. Crashes can leave status files behind that
weren't covered by the trap; clearing them defensively keeps the
dashboard truthful:

```bash
cmd_start() {
    if check_running; then show_status; exit 0; fi
    clear_all_status   # belt-and-suspenders for prior crashes
    do_fresh_start
}
```

If your dev runner backgrounds itself, run the work inside a process-
group leader so a single `kill -- -$PGID` reaps the whole tree. Bash
gives you this with `set -m`:

```bash
set -m
(
    MY_PGID=$BASHPID
    cleanup() {
        trap - EXIT INT TERM HUP
        rm -f "$REGISTRY_FILE"
        clear_all_status
        kill -- -"$MY_PGID" 2>/dev/null || true
    }
    trap cleanup EXIT INT TERM HUP

    # ... start all subprocesses ...
    wait
) > "$LOG_FILE" 2>&1 &
echo $! > "$PID_FILE"
register_instance "$!"
```

### 5. (Optional) Detect stale instances

Before starting, check whether a previous run is still alive — if not,
clean up its leftovers:

```bash
check_running() {
    [ -f "$PID_FILE" ] || return 1
    local pid=$(cat "$PID_FILE")
    if ! kill -0 "$pid" 2>/dev/null; then
        rm -f "$PID_FILE"
        unregister_instance
        return 1
    fi
    return 0
}
```

A stricter version verifies the PID is still a process-group leader,
guarding against PID recycling:

```bash
local pgid=$(ps -o pgid= -p "$pid" 2>/dev/null | tr -d ' ')
[ "$pgid" = "$pid" ] || { rm -f "$PID_FILE"; unregister_instance; return 1; }
```

## States — what to write and when

Status file content is `<state>` or `<state>: <detail>` on a single
line. Overwrite atomically (`echo "..." > file`, never `>>`) on each
state transition, not on every log line.

The canonical states, roughly in the order a unit moves through them:

- **`starting`** — you've launched the process but it isn't doing
  useful work yet (port binding, config load, importing modules).
  Optional; for fast-starting things you can skip straight to
  `running` or `ready`.

- **`building`** — work is in progress that the user is actively
  waiting on: compiling, bundling, installing dependencies. Use the
  detail to name the substep so the user can tell *what* is building
  without tailing logs: `building: cargo`, `building: wasm-pack`,
  `building: npm install`. Write this every time a watcher
  (cargo-watch, vite, esbuild) re-runs, not just on first build.

- **`running`** — the process is up. For things without a port
  (workers, watchers, background jobs) this is the terminal state.
  For servers it's an intermediate state between "started" and
  "accepting traffic" — flip to `ready` once the URL responds.

- **`ready`** — the process is fully serving. Use whenever there's a
  port and a readiness condition you can check (HTTP 200 on
  `/health`, log line `Listening on...`). Downstream tooling treats
  `ready` as "safe to hit it now."

- **`failed`** — unrecoverable error. *Always include a detail*
  saying why, in 2–5 words: `failed: exit code 1`,
  `failed: build error`, `failed: timeout`. Without the detail the
  user has to tail logs to find out what went wrong, defeating the
  point.

- **`stopped`** — you intentionally stopped the unit while the
  supervisor stays alive. Rare in practice. On full shutdown, *delete*
  the status file instead — that's how `clear_all_status` works.

The detail after `: ` is free-form. Same-state writes that only
change the detail (e.g. `building: cargo` → `building: wasm-pack`)
don't fire transition notifications, so it's safe to update the
detail aggressively without spamming the user.

For sutra's rendering of these states (color, glyph, system sound)
see [STATE_SPEC.md](../STATE_SPEC.md). As a writer you don't need to
care about that — pick the right state string and the rendering
follows.

## Common units

A typical web stack might emit:

| Unit       | Example states |
|------------|----------------|
| `server`   | `building: cargo`, `running` |
| `wasm`     | `building: wasm-pack`, `ready`, `failed: wasm-pack error` |
| `vite`     | `starting`, `ready` |
| `metro`    | `starting`, `running`, `failed: timeout` |
| `mobile`   | `building: rust bindings`, `building: xcode`, `ready` |
| `seed`     | `running`, `ready`, `failed` (self-clearing — see "transient units" above) |

Names are arbitrary — pick whatever makes sense and is short enough to
fit in a column.

## Checklist

Before declaring the integration done, verify each of these. They map
directly to bugs sutra users hit when a dev runner is partway done.

- [ ] **Stable id.** `REGISTRY_KEY` is derived from the absolute project
      path (`shasum -a 256` is fine), 16+ hex chars, no dots, no
      letters above f.
- [ ] **Meta written at start.** `~/.dev-runner/<id>` exists once the
      script reaches "servers running", with at least `DIR=` and
      `PID=`. Add `*_PORT=` for any service that has a port.
- [ ] **Status written for each subprocess.** Every long-running
      subprocess has a corresponding `<id>.<unit>.status` file that
      transitions through `starting` → `ready` (or `building` →
      `running`/`ready`).
- [ ] **`failed` is reachable.** If the build or server crashes, *some*
      unit ends up at `failed: <reason>` rather than just disappearing.
- [ ] **Trap on EXIT/INT/TERM/HUP.** Both the meta file and *all*
      `<id>.*.status` files are removed when the script exits, even
      under Ctrl+C or `kill -TERM`.
- [ ] **Defensive clear at start.** `clear_all_status` (or equivalent)
      runs before a fresh start, so a previous crash doesn't leave the
      dashboard showing stale `ready` rows.
- [ ] **`PID=` is the supervisor.** The PID written to the meta file
      leads a process group (`PGID == PID`), so sutra's "terminate"
      button kills the whole tree, not just one child.
- [ ] **Re-runs are idempotent.** Running the script twice in a row
      doesn't pile up duplicate registry entries or status files. (The
      `kill -0` liveness check + `unregister_instance` covers this.)
- [ ] **Manual smoke test.** Start the script, run
      `ls ~/.dev-runner/`, confirm one meta file + one or more
      `<id>.*.status` files. Stop the script, run `ls` again, confirm
      the entries are gone.

If `~/.dev-runner/` doesn't exist on the user's machine, your script
should `mkdir -p` it — sutra creates it lazily but doesn't require it
to exist before the first run.

## Troubleshooting

- **My env doesn't show up.** The meta filename must be hex-only with
  no `.`. `id=$(... | cut -c1-16)` is fine; `id="myproject"` is not.
- **Status row sticks around after stop.** Trap missed, or the process
  was killed with `-9` first. Add `clear_all_status` to your `--stop`
  handler too.
- **Sutra plays a sound storm on startup.** It shouldn't — sutra
  snapshots state silently on launch. If it does, file an issue with
  the contents of `~/.dev-runner/`.
- **Detail text isn't updating live.** Make sure your status writes
  are atomic — one `echo > file`, not multiple `echo >>`.
- **Two checkouts of the same project conflict.** They won't — the id
  hashes the absolute path, so `~/code/myapp` and `~/work/myapp` get
  different ids.

## See also

- [STATE_SPEC.md](../STATE_SPEC.md) — exact file format and edge cases.
- Source repo: <https://github.com/dnorman/sutra>
