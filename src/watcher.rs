use std::path::Path;
use std::sync::mpsc;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::model::state_dir;

/// Events emitted by the registry watcher.
#[derive(Debug, Clone)]
pub enum WatchEvent {
    /// An environment's meta file or status dotfile was created or modified.
    EnvironmentChanged(String),
    /// An environment's meta file or status dotfile was removed.
    EnvironmentRemoved(String),
}

/// Watches ~/.dev-runner/ for filesystem changes and emits WatchEvents.
pub struct RegistryWatcher {
    _watcher: RecommendedWatcher,
    pub rx: mpsc::Receiver<WatchEvent>,
}

/// Extract the hash ID from a filename.
/// - Meta file: "df79fed95eebc05d" → "df79fed95eebc05d"
/// - Status file: "df79fed95eebc05d.server.status" → "df79fed95eebc05d"
fn extract_hash_id(path: &Path) -> Option<String> {
    let fname = path.file_name()?.to_str()?;

    // Status files (both conventions):
    //   new: <hash>.<unit_name>.status
    //   old: .<hash>.<unit_name>.status
    if let Some(rest) = fname.strip_suffix(".status") {
        // Strip optional leading dot for old convention
        let rest = rest.strip_prefix('.').unwrap_or(rest);
        if let Some((hash, _unit)) = rest.split_once('.') {
            if !hash.is_empty() && hash.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(hash.to_string());
            }
        }
        return None;
    }

    // Meta files: just the hash (no dots, not hidden)
    if !fname.starts_with('.')
        && !fname.contains('.')
        && !fname.is_empty()
        && fname.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Some(fname.to_string());
    }

    None
}

impl RegistryWatcher {
    pub fn new() -> notify::Result<Self> {
        let Some(dir) = state_dir() else {
            return Err(notify::Error::generic("could not determine home directory"));
        };

        // Ensure the directory exists
        std::fs::create_dir_all(&dir).ok();

        let (tx, rx) = mpsc::channel();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            let Ok(event) = res else { return };

            for path in &event.paths {
                let Some(id) = extract_hash_id(path) else {
                    continue;
                };

                let watch_event = match event.kind {
                    EventKind::Remove(_) => WatchEvent::EnvironmentRemoved(id),
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        WatchEvent::EnvironmentChanged(id)
                    }
                    _ => continue,
                };

                // Best-effort send; if the receiver is gone, silently drop.
                let _ = tx.send(watch_event);
            }
        })?;

        watcher.watch(&dir, RecursiveMode::NonRecursive)?;

        Ok(RegistryWatcher {
            _watcher: watcher,
            rx,
        })
    }
}
