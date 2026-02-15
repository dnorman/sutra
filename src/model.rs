use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use nix::sys::signal;
use nix::unistd::Pid;

/// State of a unit
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    None,           // empty/missing status
    Starting,
    Building,
    Running,
    Ready,
    Failed,
    Stopped,
    Other(String),  // unrecognized state, stored as-is
}

impl State {
    pub fn parse(s: &str) -> State {
        match s {
            "starting" => State::Starting,
            "building" => State::Building,
            "running" => State::Running,
            "ready" => State::Ready,
            "failed" => State::Failed,
            "stopped" => State::Stopped,
            _ => State::Other(s.to_string()),
        }
    }

    pub fn display_indicator(&self) -> &str {
        match self {
            State::None => "\u{25cb}",      // ○
            State::Starting => "\u{25cc}",   // ◌
            State::Building => "\u{25d1}",   // ◑
            State::Running => "\u{25cf}",    // ●
            State::Ready => "\u{25cf}",      // ●
            State::Failed => "\u{2717}",     // ✗
            State::Stopped => "\u{25cb}",    // ○
            State::Other(_) => "\u{25c6}",   // ◆
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, State::Starting | State::Building | State::Running | State::Ready)
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            State::None => "",
            State::Starting => "starting",
            State::Building => "building",
            State::Running => "running",
            State::Ready => "ready",
            State::Failed => "failed",
            State::Stopped => "stopped",
            State::Other(s) => s.as_str(),
        };
        write!(f, "{}", label)
    }
}

/// Status of a single named unit (e.g., "server", "vite")
#[derive(Debug, Clone)]
pub struct UnitStatus {
    pub name: String,
    pub state: State,
    pub detail: Option<String>,
}

impl UnitStatus {
    /// Parse a status value like "building: Compiling Rust bindings".
    /// The `name` comes from the status filename, not the content.
    /// Uses State::None for empty content.
    pub fn parse(name: &str, content: &str) -> UnitStatus {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return UnitStatus {
                name: name.to_string(),
                state: State::None,
                detail: None,
            };
        }

        let (state_str, detail) = match trimmed.split_once(": ") {
            Some((s, d)) => (s, Some(d.to_string())),
            None => (trimmed, None),
        };

        let state = State::parse(state_str);
        UnitStatus {
            name: name.to_string(),
            state,
            detail,
        }
    }
}

/// A registered environment instance
#[derive(Debug, Clone)]
pub struct Environment {
    pub id: String,
    pub dir: PathBuf,
    pub pid: u32,
    pub ports: HashMap<String, u16>, // lowercase unit name → port
    pub started: u64,
    pub alive: bool,
    pub units: Vec<UnitStatus>,
}

impl Environment {
    /// Load an environment instance from its meta file.
    ///
    /// Meta file: `~/.dev-runner/<hash>` (KEY=VALUE lines)
    /// Status files: `~/.dev-runner/<hash>.<unit_name>.status` (single line: `<state>[: <detail>]`)
    pub fn load(meta_path: &Path) -> Option<Environment> {
        let content = fs::read_to_string(meta_path).ok()?;
        let id = meta_path.file_name()?.to_str()?.to_string();

        let mut dir = None;
        let mut pid = None;
        let mut ports = HashMap::new();
        let mut started = None;

        for line in content.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "DIR" => dir = Some(PathBuf::from(value)),
                "PID" => pid = value.parse().ok(),
                "STARTED" => started = value.parse().ok(),
                k if k.ends_with("_PORT") => {
                    if let Ok(port) = value.parse::<u16>() {
                        let name = k.strip_suffix("_PORT").unwrap().to_lowercase();
                        ports.insert(name, port);
                    }
                }
                _ => {}
            }
        }

        let pid = pid?;
        let alive = match i32::try_from(pid) {
            Ok(raw_pid) => signal::kill(Pid::from_raw(raw_pid), None).is_ok(),
            Err(_) => false,
        };

        // Scan for status files in both conventions:
        //   new: <hash>.<unit_name>.status
        //   old: .<hash>.<unit_name>.status
        let parent = meta_path.parent()?;
        let new_prefix = format!("{}.", id);
        let old_prefix = format!(".{}.", id);
        let mut units = Vec::new();

        if let Ok(entries) = fs::read_dir(parent) {
            for entry in entries.flatten() {
                let fname = entry.file_name();
                let fname_str = fname.to_string_lossy();
                let rest = fname_str
                    .strip_prefix(&new_prefix)
                    .or_else(|| fname_str.strip_prefix(&old_prefix));
                if let Some(rest) = rest {
                    if let Some(unit_name) = rest.strip_suffix(".status") {
                        if !unit_name.is_empty() {
                            if let Ok(status_content) = fs::read_to_string(entry.path()) {
                                units.push(UnitStatus::parse(unit_name, &status_content));
                            }
                        }
                    }
                }
            }
        }

        units.sort_by(|a, b| a.name.cmp(&b.name));

        Some(Environment {
            id,
            dir: dir?,
            pid,
            ports,
            started: started.unwrap_or(0),
            alive,
            units,
        })
    }

    /// Look up the port associated with a unit by name.
    /// Matches against `*_PORT` keys from the meta file (e.g. `SERVER_PORT` → "server").
    pub fn port_for(&self, unit_name: &str) -> Option<u16> {
        self.ports.get(unit_name).copied()
    }

    /// Short display name derived from the project directory.
    pub fn display_name(&self) -> &str {
        self.dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
    }

    /// Elapsed time since started, as a human-readable string.
    pub fn elapsed_string(&self) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let elapsed = now.saturating_sub(self.started);
        if elapsed < 60 {
            format!("{}s", elapsed)
        } else if elapsed < 3600 {
            format!("{}m", elapsed / 60)
        } else if elapsed < 86400 {
            format!("{}h {}m", elapsed / 3600, (elapsed % 3600) / 60)
        } else {
            format!("{}d", elapsed / 86400)
        }
    }
}

/// Returns the path to the dev-runner registry directory (~/.dev-runner/),
/// or `None` if the home directory cannot be determined.
pub fn state_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".dev-runner"))
}

/// Returns true if a filename looks like a hex-only meta file (no dots, not hidden).
fn is_meta_file(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains('.')
        && name.chars().all(|c| c.is_ascii_hexdigit())
}

/// Load all environment instances from the registry.
pub fn load_all() -> Vec<Environment> {
    let Some(dir) = state_dir() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut envs = Vec::new();
    for entry in entries.flatten() {
        let fname = entry.file_name();
        let fname_str = fname.to_string_lossy();
        if is_meta_file(&fname_str) {
            if let Some(env) = Environment::load(&entry.path()) {
                envs.push(env);
            }
        }
    }

    envs.sort_by(|a, b| a.dir.cmp(&b.dir));
    envs
}
