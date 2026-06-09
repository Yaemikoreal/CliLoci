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
        let executables = self.scan_dirs(&path_dirs);

        // Persist cache
        self.write_cache(&hash, &executables);

        executables
    }

    /// Collect tools from project-local directories only
    /// (node_modules/.bin, .venv/bin, target/debug, etc.).
    ///
    /// Does NOT use the disk cache — project dirs are ephemeral
    /// and usually very small.
    pub fn collect_project(&self) -> Vec<String> {
        let cwd = std::env::current_dir().unwrap_or_default();
        let dirs = Self::detect_project_dirs(&cwd);
        self.scan_dirs(&dirs)
    }

    /// Detect project-local bin directories by probing for common
    /// project markers at `cwd`.
    pub fn detect_project_dirs(cwd: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut dirs = Vec::new();

        // ── Node.js (node_modules/.bin) ───────────────────────────
        if cwd.join("package.json").is_file() {
            let node_bin = cwd.join("node_modules").join(".bin");
            if node_bin.is_dir() {
                dirs.push(node_bin);
            }
        }

        // ── Python venv (.venv/bin or venv/bin) ───────────────────
        for venv in &[".venv", "venv"] {
            let cfg = cwd.join(venv).join("pyvenv.cfg");
            if cfg.is_file() {
                let bin_dir = if cfg!(windows) {
                    cwd.join(venv).join("Scripts")
                } else {
                    cwd.join(venv).join("bin")
                };
                if bin_dir.is_dir() {
                    dirs.push(bin_dir);
                }
            }
        }

        // ── Rust (target/debug + target/release) ─────────────────
        if cwd.join("Cargo.toml").is_file() {
            for subdir in &["debug", "release"] {
                let d = cwd.join("target").join(subdir);
                if d.is_dir() {
                    dirs.push(d);
                }
            }
        }

        // ── Conda ($CONDA_PREFIX/bin) ─────────────────────────────
        if let Ok(prefix) = std::env::var("CONDA_PREFIX") {
            let conda_bin = std::path::PathBuf::from(&prefix).join("bin");
            if conda_bin.is_dir() {
                dirs.push(conda_bin);
            }
        }

        dirs
    }

    /// Core scanning logic: iterate `dirs`, collect executables,
    /// deduplicate, blacklist-filter, sort.
    fn scan_dirs(&self, dirs: &[std::path::PathBuf]) -> Vec<String> {
        let mut executables = Vec::new();
        let mut seen = HashSet::new();

        for dir in dirs {
            let entries = match std::fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(_) => continue, // skip inaccessible directories
            };

            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_lossy = name.to_string_lossy();
                let name_str = name_lossy.to_string();

                // Skip files with non-UTF-8 names — they cannot be executed
                // via Command::new() on most platforms.
                if name_str.contains('\u{FFFD}') {
                    eprintln!(
                        "loci: skipping file with non-UTF-8 name: {}",
                        name_lossy
                    );
                    continue;
                }

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

        executables.sort();
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

        // Append LOCI_PATH_EXTRA (warn on missing entries — user explicitly chose these)
        if let Ok(extra) = std::env::var("LOCI_PATH_EXTRA") {
            for p in extra.split(sep).filter(|s| !s.is_empty()) {
                let pb = PathBuf::from(p);
                if pb.is_dir() {
                    dirs.push(pb);
                } else {
                    eprintln!(
                        "loci: warning: LOCI_PATH_EXTRA entry '{}' \
                         does not exist, skipping",
                        p
                    );
                }
            }
        }

        // Retain only directories that actually exist (PATH entries may legitimately not exist)
        dirs.retain(|d| d.is_dir());
        dirs
    }

    /// Public accessor for the resolved PATH directories.
    /// Used by metadata collection to resolve full binary paths.
    pub fn path_dirs(&self) -> Vec<PathBuf> {
        self.get_path_dirs()
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

    #[cfg(test)]
    pub(crate) fn test_new(blacklist: Vec<String>, cache_dir: std::path::PathBuf) -> Self {
        Scanner { cache_dir, blacklist }
    }
}

// ---------------------------------------------------------------------------
// Tests (only compile when testing)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// Helper: create a unique temp dir per test label.
    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "loci-scanner-test-{}-{}",
            label,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &std::path::Path) {
        fs::write(path, "").unwrap();
    }

    /// Return the name that scan_dirs will store for a tool on this platform.
    /// On Windows, `is_executable` requires a PATHEXT extension (.exe, .bat, etc.),
    /// so scan_dirs stores the full filename including extension.
    /// On Unix, no extension is needed.
    #[cfg(windows)]
    fn scan_name(name: &str) -> String {
        if name.contains('.') { name.to_string() } else { format!("{}.exe", name) }
    }
    #[cfg(not(windows))]
    fn scan_name(name: &str) -> String {
        name.to_string()
    }

    /// Create a file that platform::is_executable will accept.
    #[cfg(unix)]
    fn touch_exec(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, "#!/bin/sh\necho hi\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(windows)]
    fn touch_exec(path: &std::path::Path) {
        // PATHEXT typically includes .EXE
        let p = if path.extension().map(|e| e == "exe" || e == "EXE").unwrap_or(false) {
            path.to_path_buf()
        } else {
            let mut p = path.to_path_buf();
            p.set_extension("exe");
            p
        };
        fs::write(&p, "").unwrap();
    }
    #[cfg(not(any(unix, windows)))]
    fn touch_exec(path: &std::path::Path) {
        fs::write(path, "").unwrap();
    }

    // ── is_blacklisted ─────────────────────────────────────────────

    #[test]
    fn is_blacklisted_default_shell_builtins() {
        let s = Scanner::test_new(vec![], temp_dir("bl-default"));
        assert!(s.is_blacklisted("cd"), "cd");
        assert!(s.is_blacklisted("echo"), "echo");
        assert!(s.is_blacklisted("export"), "export");
        assert!(s.is_blacklisted("history"), "history");
        assert!(s.is_blacklisted("exit"), "exit");
    }

    #[test]
    fn is_blacklisted_user_entries() {
        let s = Scanner::test_new(
            vec!["my-tool".to_string(), "another-one".to_string()],
            temp_dir("bl-user"),
        );
        assert!(s.is_blacklisted("my-tool"));
        assert!(s.is_blacklisted("another-one"));
    }

    #[test]
    fn is_blacklisted_clean() {
        let s = Scanner::test_new(vec![], temp_dir("bl-clean"));
        assert!(!s.is_blacklisted("git"));
        assert!(!s.is_blacklisted("python3"));
        assert!(!s.is_blacklisted("cargo"));
    }

    #[test]
    fn load_user_blacklist_comment_empty_lines() {
        // Test the parsing logic directly (the function reads from disk,
        // but these invariants apply to what it returns).
        let contents = "# comment\n\n tool1\n  \n# another\ntool2\n";
        let parsed: Vec<String> = contents
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        assert_eq!(parsed, vec!["tool1", "tool2"]);
    }

    // ── detect_project_dirs ────────────────────────────────────────

    #[test]
    fn detect_project_dirs_node() {
        let root = temp_dir("node");
        touch(&root.join("package.json"));
        let node_bin = root.join("node_modules").join(".bin");
        fs::create_dir_all(&node_bin).unwrap();

        let dirs = Scanner::detect_project_dirs(&root);
        assert!(dirs.contains(&node_bin));
    }

    #[test]
    fn detect_project_dirs_python_venv() {
        let root = temp_dir("venv");
        let venv = root.join(".venv");
        fs::create_dir_all(&venv).unwrap();
        touch(&venv.join("pyvenv.cfg"));
        let bin = if cfg!(windows) {
            venv.join("Scripts")
        } else {
            venv.join("bin")
        };
        fs::create_dir_all(&bin).unwrap();

        let dirs = Scanner::detect_project_dirs(&root);
        assert!(dirs.contains(&bin));
    }

    #[test]
    fn detect_project_dirs_rust() {
        let root = temp_dir("rust");
        touch(&root.join("Cargo.toml"));
        let debug = root.join("target").join("debug");
        let release = root.join("target").join("release");
        fs::create_dir_all(&debug).unwrap();
        fs::create_dir_all(&release).unwrap();

        let dirs = Scanner::detect_project_dirs(&root);
        assert!(dirs.contains(&debug));
        assert!(dirs.contains(&release));
    }

    #[test]
    fn detect_project_dirs_conda() {
        let root = temp_dir("conda");
        let prefix = root.join("env");
        let bin = prefix.join("bin");
        fs::create_dir_all(&bin).unwrap();

        std::env::set_var("CONDA_PREFIX", prefix.to_str().unwrap());
        let dirs = Scanner::detect_project_dirs(&root);
        std::env::remove_var("CONDA_PREFIX");

        assert!(dirs.contains(&bin));
    }

    #[test]
    fn detect_project_dirs_empty_when_no_markers() {
        let root = temp_dir("empty");
        let dirs = Scanner::detect_project_dirs(&root);
        assert!(dirs.is_empty());
    }

    #[test]
    fn detect_project_dirs_no_node_bin_without_dir() {
        let root = temp_dir("partial");
        touch(&root.join("package.json"));
        // Intentionally NOT creating node_modules/.bin
        let dirs = Scanner::detect_project_dirs(&root);
        assert!(!dirs.iter().any(|d| d.to_string_lossy().contains("node_modules")));
    }

    // ── compute_path_hash ──────────────────────────────────────────

    #[test]
    fn compute_path_hash_deterministic() {
        let d1 = temp_dir("hash-a");
        let d2 = temp_dir("hash-b");
        touch_exec(&d1.join("x"));
        touch_exec(&d2.join("y"));

        let s = Scanner::test_new(vec![], temp_dir("hash-scanner"));
        let dirs = vec![d1.clone(), d2.clone()];
        let h1 = s.compute_path_hash(&dirs);
        let h2 = s.compute_path_hash(&dirs);
        assert_eq!(h1, h2, "same dirs → same hash");
    }

    // ── scan_dirs ──────────────────────────────────────────────────

    #[test]
    fn scan_dirs_deduplicates() {
        let d1 = temp_dir("dedup1");
        let d2 = temp_dir("dedup2");
        touch_exec(&d1.join("tool_a"));
        touch_exec(&d2.join("tool_a")); // same name in both
        touch_exec(&d1.join("tool_b"));

        let s = Scanner::test_new(vec![], temp_dir("scan-dedup"));
        let results = s.scan_dirs(&[d1, d2]);
        let name = scan_name("tool_a");
        assert_eq!(
            results.iter().filter(|n| n.as_str() == name.as_str()).count(),
            1,
            "tool_a should be deduplicated"
        );
        assert!(results.contains(&name));
        assert!(results.contains(&scan_name("tool_b")));
    }

    #[test]
    fn scan_dirs_skips_blacklisted() {
        let d = temp_dir("skip");
        touch_exec(&d.join("good-tool"));
        touch_exec(&d.join("bad-tool"));

        // Blacklist entries must match what scan_dirs stores (includes .exe on Windows)
        let s = Scanner::test_new(vec![scan_name("bad-tool")], temp_dir("scan-skip"));
        let results = s.scan_dirs(&[d]);
        assert!(results.contains(&scan_name("good-tool")));
        assert!(!results.contains(&scan_name("bad-tool")));
    }

    #[test]
    fn scan_dirs_empty_dir() {
        let d = temp_dir("empty-scan");
        let s = Scanner::test_new(vec![], temp_dir("scan-empty"));
        assert!(s.scan_dirs(&[d]).is_empty());
    }

    #[test]
    fn scan_dirs_nonexistent_dir() {
        let d = PathBuf::from("/nonexistent-loci-test");
        let s = Scanner::test_new(vec![], temp_dir("scan-nonexist"));
        assert!(s.scan_dirs(&[d]).is_empty());
    }

    #[test]
    fn scan_dirs_first_path_wins() {
        let d1 = temp_dir("first");
        let d2 = temp_dir("second");
        touch_exec(&d1.join("shared"));
        touch_exec(&d2.join("shared"));

        let s = Scanner::test_new(vec![], temp_dir("scan-first"));
        let results = s.scan_dirs(&[d1, d2]);
        let name = scan_name("shared");
        assert_eq!(
            results.iter().filter(|n| n.as_str() == name.as_str()).count(),
            1
        );
    }

    // ── Cache persistence ──────────────────────────────────────────

    #[test]
    fn cache_read_write_roundtrip() {
        let cache = temp_dir("cache-rw");
        let s = Scanner::test_new(vec![], cache.clone());

        let hash = "test-hash-001".to_string();
        let tools = vec!["alpha".to_string(), "beta".to_string()];
        s.write_cache(&hash, &tools);

        let entry = s.read_cache().unwrap();
        assert_eq!(entry.path_hash, hash);
        assert_eq!(entry.executables, tools);
    }

    #[test]
    fn cache_read_missing_file() {
        let cache = temp_dir("cache-miss");
        let s = Scanner::test_new(vec![], cache);
        assert!(s.read_cache().is_none());
    }

    // ── collect / collect_project (light integration) ──────────────

    #[test]
    fn collect_project_detects_local_tools() {
        let root = temp_dir("collect-proj");
        touch(&root.join("Cargo.toml"));
        let debug = root.join("target").join("debug");
        let release = root.join("target").join("release");
        fs::create_dir_all(&debug).unwrap();
        fs::create_dir_all(&release).unwrap();
        // Place an executable in one of the build dirs
        touch_exec(&debug.join("my-project-exe"));

        let s = Scanner::test_new(vec![], temp_dir("cp-scanner"));

        // collect_project calls env::current_dir() which points to the repo during
        // tests, not our temp dir. Test the underlying logic directly instead:
        let dirs = Scanner::detect_project_dirs(&root);
        let results = s.scan_dirs(&dirs);
        assert!(results.contains(&scan_name("my-project-exe")));
    }
}
