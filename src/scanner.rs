use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::platform;

/// Default blacklist of shell builtins / non-interactive utilities that
/// clutter the listing without adding value.
const DEFAULT_BLACKLIST: &[&str] = &[
    "[", "alias", "bg", "bind", "break", "builtin", "caller", "case", "cd",
    "command", "compgen", "complete", "continue", "declare", "dirs", "disown",
    "echo", "enable", "eval", "exec", "exit", "export", "false", "fc", "fg",
    "getopts", "hash", "help", "history", "jobs", "kill", "let", "local",
    "logout", "mapfile", "popd", "pushd", "pwd", "read", "readarray", "return",
    "set", "shift", "shopt", "source", "suspend", "test", "times", "trap",
    "true", "type", "typeset", "ulimit", "umask", "unalias", "unset", "wait",
];

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    path_hash: String,
    executables: Vec<String>,
}

pub struct Scanner {
    cache_dir: PathBuf,
    blacklist: Vec<String>,
}

impl Scanner {
    pub fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("loci");

        let blacklist = Self::load_user_blacklist();

        Scanner {
            cache_dir,
            blacklist,
        }
    }

    /// Collect all executables from PATH. Uses a disk cache keyed by a
    /// hash of PATH entries + their mtimes to skip rescanning when
    /// nothing changed.
    pub fn collect(&self) -> Vec<String> {
        let path_dirs = self.get_path_dirs();
        let hash = self.compute_path_hash(&path_dirs);

        // Try cache first (skip if user blacklist changed since last scan)
        if let Some(cached) = self.read_cache() {
            if cached.path_hash == hash {
                return cached.executables;
            }
        }

        // Scan PATH directories
        let mut executables = Vec::new();
        let mut seen = HashSet::new();

        for dir in &path_dirs {
            let entries = match std::fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(_) => continue, // skip inaccessible directories
            };

            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy().to_string();

                if seen.contains(&name_str) {
                    continue; // first PATH entry wins
                }

                // Check against default blacklist + user blacklist
                if self.is_blacklisted(&name_str) {
                    continue;
                }

                match entry.metadata() {
                    Ok(meta) if platform::is_executable(&name_str, &meta) => {
                        seen.insert(name_str.clone());
                        executables.push(name_str);
                    }
                    _ => {}
                }
            }
        }

        // Sort alphabetically for browsability
        executables.sort();

        // Persist cache
        self.write_cache(&hash, &executables);

        executables
    }

    /// Check both the built-in default blacklist and the user's custom
    /// blacklist file.
    fn is_blacklisted(&self, name: &str) -> bool {
        DEFAULT_BLACKLIST.contains(&name) || self.blacklist.iter().any(|b| b == name)
    }

    /// Read the user blacklist from `~/.config/loci/blacklist`
    /// (or platform equivalent). One name per line; blank lines and
    /// `#`-prefixed comments are ignored.
    fn load_user_blacklist() -> Vec<String> {
        let config_dir = match dirs::config_dir() {
            Some(d) => d.join("loci"),
            None => return Vec::new(),
        };
        let path = config_dir.join("blacklist");
        match std::fs::read_to_string(&path) {
            Ok(contents) => contents
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Read `PATH` and `LOCI_PATH_EXTRA`, return existing directories.
    fn get_path_dirs(&self) -> Vec<PathBuf> {
        let path = std::env::var("PATH").unwrap_or_default();
        let sep = if cfg!(windows) { ';' } else { ':' };

        let mut dirs: Vec<PathBuf> = path
            .split(sep)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();

        // Append LOCI_PATH_EXTRA
        if let Ok(extra) = std::env::var("LOCI_PATH_EXTRA") {
            for p in extra.split(sep).filter(|s| !s.is_empty()) {
                dirs.push(PathBuf::from(p));
            }
        }

        // Retain only directories that actually exist
        dirs.retain(|d| d.is_dir());
        dirs
    }

    /// Compute a SHA-256 fingerprint of all PATH directories and their
    /// modification times. A change in any directory's mtime (e.g. a new
    /// executable installed) invalidates the cache.
    fn compute_path_hash(&self, dirs: &[PathBuf]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for dir in dirs {
            hasher.update(dir.to_string_lossy().as_bytes());
            if let Ok(meta) = std::fs::metadata(dir) {
                if let Ok(mtime) = meta.modified() {
                    hasher.update(format!("{:?}", mtime).as_bytes());
                }
            }
        }
        format!("{:x}", hasher.finalize())
    }

    fn cache_path(&self) -> PathBuf {
        self.cache_dir.join("cache.json")
    }

    fn read_cache(&self) -> Option<CacheEntry> {
        let path = self.cache_path();
        if !path.exists() {
            return None;
        }
        let data = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&data).ok()
    }

    fn write_cache(&self, hash: &str, executables: &[String]) {
        let _ = std::fs::create_dir_all(&self.cache_dir);
        let entry = CacheEntry {
            path_hash: hash.to_string(),
            executables: executables.to_vec(),
        };
        if let Ok(json) = serde_json::to_string(&entry) {
            // Atomic write: write to temp, then rename
            let tmp = self.cache_dir.join("cache.json.tmp");
            let _ = std::fs::write(&tmp, &json);
            let _ = std::fs::rename(&tmp, self.cache_path());
        }
    }
}
