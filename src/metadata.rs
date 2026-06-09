//! Minimal metadata layer for `loci --meta` output.
//!
//! Provides on-demand tool metadata (version, category, source path)
//! without adding external dependencies. Category inference is done
//! entirely by name-pattern matching — no runtime config required.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMeta {
    pub version: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub path: String,
}

/// Load user-defined tag mappings from `~/.config/loci/tags.json`.
/// Format: { "tool-name": ["tag1", "tag2"] }
pub fn load_user_tags() -> HashMap<String, Vec<String>> {
    let config_dir = match dirs::config_dir() {
        Some(d) => d.join("loci"),
        None => return HashMap::new(),
    };
    let path = config_dir.join("tags.json");
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

/// Collect metadata for each executable in the list.
///
/// * `executables` — tool names returned by the scanner.
/// * `path_dirs` — PATH directories (used to resolve the full binary path).
/// * `with_versions` — when true, probes each tool's `--version` / `-V`
///   (spawns a process per tool; use sparingly).
/// * `user_tags` — optional user-defined tag overrides from tags.json.
pub fn collect_meta(
    executables: &[String],
    path_dirs: &[impl AsRef<Path>],
    with_versions: bool,
    user_tags: Option<&HashMap<String, Vec<String>>>,
) -> HashMap<String, ToolMeta> {
    let mut meta = HashMap::with_capacity(executables.len());
    for name in executables {
        let source = find_path(name, path_dirs);
        let category = infer_category(name);
        let version = if with_versions {
            probe_version(name, &source)
        } else {
            None
        };

        // Build tags: built-in category + user-defined tags
        let mut tags: Vec<String> = Vec::new();
        if let Some(cat) = category {
            tags.push(cat.to_string());
        }
        if let Some(ut) = user_tags {
            if let Some(extra) = ut.get(name.as_str()) {
                for t in extra {
                    if !tags.contains(t) {
                        tags.push(t.clone());
                    }
                }
            }
        }

        meta.insert(
            name.clone(),
            ToolMeta {
                version,
                category: category.map(|c| c.to_string()),
                tags,
                path: source,
            },
        );
    }
    meta
}

/// Walk `path_dirs` in order and return the first path where `name` exists
/// as a file.  On Windows the `PATHEXT` extensions are tried automatically.
fn find_path(name: &str, path_dirs: &[impl AsRef<Path>]) -> String {
    for dir in path_dirs {
        let full = dir.as_ref().join(name);
        if full.is_file() {
            return full.to_string_lossy().to_string();
        }

        // Windows: try appending each PATHEXT extension.
        #[cfg(windows)]
        {
            let path_ext = std::env::var("PATHEXT")
                .unwrap_or_else(|_| ".EXE;.BAT;.CMD;.COM;.PS1".to_string());
            for ext in path_ext.split(';') {
                let with_ext = dir.as_ref().join(format!("{}{}", name, ext));
                if with_ext.is_file() {
                    return with_ext.to_string_lossy().to_string();
                }
            }
        }
    }
    String::new()
}

/// Tools that are known to be GUI-only and should never be probed
/// for version information.  Spawning them with `--version` would
/// open unwanted windows (gitk, gvim, etc.) or hang indefinitely.
const VERSION_PROBE_BLACKLIST: &[&str] = &[
    "gitk",      // Tcl/Tk Git GUI — opens a window on any arg
    "git-gui",   // Tcl/Tk Git GUI component
    "gvim",      // GUI Vim — opens a window even with --version
];

/// Try `--version` first, then `-V`, and return the first non-empty output
/// line.  Spawns a subprocess with a 3-second timeout per flag —
/// intended for `--meta` mode only.
///
/// Known GUI-only tools (gitk, gvim, etc.) are silently skipped.
fn probe_version(name: &str, resolved_path: &str) -> Option<String> {
    // Strip .exe for blacklist check (Windows compat).
    let probe_name = name.strip_suffix(".exe").unwrap_or(name);
    if VERSION_PROBE_BLACKLIST.contains(&probe_name) {
        return None;
    }

    // Use the resolved path when available (avoids re-searching PATH).
    let target = if resolved_path.is_empty() { name } else { resolved_path };

    for flag in &["--version", "-V"] {
        let mut cmd = std::process::Command::new(target);
        cmd.arg(flag)
            .stdin(std::process::Stdio::null())  // detach from parent console
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        // On Windows, suppress console windows for spawned processes.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        if let Ok(mut child) = cmd.spawn() {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            let mut timed_out = false;

            // Poll for child exit; use try_wait to avoid blocking.
            // IMPORTANT: Once try_wait() returns Ok(Some(_)) the child has been
            // reaped — do NOT call wait() / wait_with_output() afterwards.
            // Instead, read the remaining data from the stdout pipe directly.
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break, // exited normally, read stdout below
                    Ok(None) => {
                        if std::time::Instant::now() >= deadline {
                            let _ = child.kill();
                            let _ = child.wait();
                            timed_out = true;
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    Err(_) => {
                        timed_out = true;
                        break;
                    }
                }
            }

            if !timed_out {
                // Read stdout from the (already-waited) child process.
                let output = child.stdout.take().and_then(|mut s| {
                    use std::io::Read;
                    let mut buf = Vec::new();
                    s.read_to_end(&mut buf).ok().map(|_| buf)
                });
                if let Some(ref buf) = output {
                    let stdout = String::from_utf8_lossy(buf);
                    for line in stdout.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() && trimmed.len() < 120 {
                            return Some(trimmed.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Infer a tool category from its name (name-prefix / exact-match rules).
///
/// This is a best-effort heuristic — no external data sources are used.
pub fn infer_category(name: &str) -> Option<&'static str> {
    // Strip .exe for Windows compatibility before pattern matching.
    let name = name.strip_suffix(".exe").unwrap_or(name);

    match name {
        // ── SCM / VCS ─────────────────────────────────────────────
        n if n.starts_with("git") => Some("scm"),
        n if n == "hg" || n == "svn" || n == "jj" => Some("scm"),

        // ── Container / orchestration ───────────────────────────────
        n if n.starts_with("docker") => Some("container"),
        n if ["podman", "nerdctl", "kubectl", "minikube", "kind", "helm",
               "k9s", "ctr", "buildah", "skopeo"].contains(&n) => Some("container"),

        // ── Python ecosystem ────────────────────────────────────────
        n if n.starts_with("python")
            || n == "pip" || n.starts_with("pip")
            || n.starts_with("poetry")
            || n == "virtualenv" || n == "conda"
            || n == "mamba" || n == "uv" || n == "rye"
            || n == "pdm" || n == "hatch" => Some("python"),

        // ── Node / JavaScript ───────────────────────────────────────
        n if n.starts_with("node")
            || n == "npm" || n == "npx"
            || n.starts_with("yarn") || n.starts_with("pnpm")
            || n == "bun" || n == "deno" => Some("node"),

        // ── Compress / archive ──────────────────────────────────────
        n if ["zip", "unzip", "7z", "7za", "7zr", "tar", "gzip",
               "gunzip", "zstd", "xz", "unxz", "bzip2", "bunzip2",
               "lz4", "lzma", "unlzma", "zpaq", "p7zip", "rar",
               "unrar", "arj", "compress", "compress-pdf"]
               .contains(&n) => Some("compress"),

        // ── Network / remote ────────────────────────────────────────
        n if ["curl", "wget", "nc", "ncat", "ssh", "scp", "sftp",
               "rsync", "telnet", "tcpdump", "ping", "netstat",
               "ifconfig", "ip", "dig", "nslookup", "nmap",
               "socat", "mosh", "wg", "wg-quick", "iperf",
               "iperf3", "httpie", "xh", "aria2c"].contains(&n) => Some("network"),

        // ── Editor / IDE ────────────────────────────────────────────
        n if ["vim", "nvim", "emacs", "nano", "vi", "code", "codium",
               "zed", "helix", "kate", "neovim", "micro",
               "subl", "geany", "gvim", "xed"].contains(&n) => Some("editor"),

        // ── Rust ────────────────────────────────────────────────────
        n if n == "cargo" || n == "rustc" || n == "rustup"
            || n == "rustfmt" || n == "clippy-driver"
            || n.starts_with("cargo-") => Some("rust"),

        // ── Go ──────────────────────────────────────────────────────
        n if n == "go" || n.starts_with("gopls") || n == "gofmt"
            || n == "staticcheck" || n == "revive" || n == "golangci-lint"
            || n.starts_with("go-") => Some("go"),

        // ── Database ────────────────────────────────────────────────
        n if ["mysql", "psql", "sqlite3", "redis-cli", "redis-server",
               "mongosh", "mongo", "pg_dump", "pg_restore",
               "pg_isready", "mycli", "pgcli", "dolt",
               "sqlcmd", "sqlpackage"].contains(&n) => Some("database"),

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests (only compile when testing)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a unique temp dir for a test.
    fn temp_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "loci-meta-test-{}-{}",
            label,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Create an executable script that echoes a version string.
    #[cfg(unix)]
    fn write_version_script(dir: &std::path::Path, name: &str, version: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\necho '{}'\n", version)).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
    #[cfg(windows)]
    fn write_version_script(dir: &std::path::Path, name: &str, version: &str) -> std::path::PathBuf {
        let path = dir.join(format!("{}.bat", name));
        fs::write(&path, format!("@echo {}\r\n", version)).unwrap();
        path
    }
    #[cfg(not(any(unix, windows)))]
    fn write_version_script(dir: &std::path::Path, name: &str, version: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, version).unwrap();
        path
    }

    #[test]
    fn test_infer_category_basic() {
        assert_eq!(infer_category("git"), Some("scm"));
        assert_eq!(infer_category("git-lfs"), Some("scm"));
        assert_eq!(infer_category("python3"), Some("python"));
        assert_eq!(infer_category("pip"), Some("python"));
        assert_eq!(infer_category("docker"), Some("container"));
        assert_eq!(infer_category("curl"), Some("network"));
        assert_eq!(infer_category("vim"), Some("editor"));
        assert_eq!(infer_category("tar"), Some("compress"));
        assert_eq!(infer_category("cargo"), Some("rust"));
        assert_eq!(infer_category("cargo-clippy"), Some("rust"));
    }

    #[test]
    fn test_infer_category_windows_ext() {
        assert_eq!(infer_category("git.exe"), Some("scm"));
        assert_eq!(infer_category("python.exe"), Some("python"));
    }

    #[test]
    fn test_infer_category_unknown() {
        assert_eq!(infer_category("foobar123"), None);
        assert_eq!(infer_category("zzz"), None);
    }

    #[test]
    fn test_find_path_nonexistent() {
        let dirs: &[&Path] = &[];
        let result = find_path("does-not-exist-hopefully", dirs);
        assert!(result.is_empty());
    }

    // ── infer_category edge cases ─────────────────────────────────

    #[test]
    fn infer_category_additional_cases() {
        assert_eq!(infer_category("kubectl"), Some("container"));
        assert_eq!(infer_category("helm"), Some("container"));
        assert_eq!(infer_category("node"), Some("node"));
        assert_eq!(infer_category("bun"), Some("node"));
        assert_eq!(infer_category("deno"), Some("node"));
        assert_eq!(infer_category("go"), Some("go"));
        assert_eq!(infer_category("gopls"), Some("go"));
        assert_eq!(infer_category("7z"), Some("compress"));
        assert_eq!(infer_category("zstd"), Some("compress"));
        assert_eq!(infer_category("ssh"), Some("network"));
        assert_eq!(infer_category("rsync"), Some("network"));
        assert_eq!(infer_category("nvim"), Some("editor"));
        assert_eq!(infer_category("code"), Some("editor"));
        assert_eq!(infer_category("hg"), Some("scm"));
        assert_eq!(infer_category("svn"), Some("scm"));
        assert_eq!(infer_category("psql"), Some("database"));
        assert_eq!(infer_category("redis-cli"), Some("database"));
        assert_eq!(infer_category("rustc"), Some("rust"));
        assert_eq!(infer_category("cargo-clippy"), Some("rust"));
    }

    #[test]
    fn infer_category_node_exact() {
        assert_eq!(infer_category("npm"), Some("node"));
        assert_eq!(infer_category("npx"), Some("node"));
    }

    #[test]
    fn infer_category_python_exact() {
        assert_eq!(infer_category("conda"), Some("python"));
        assert_eq!(infer_category("uv"), Some("python"));
        assert_eq!(infer_category("mamba"), Some("python"));
    }

    // ── find_path ─────────────────────────────────────────────────

    #[test]
    fn find_path_first_dir_wins() {
        let d1 = temp_dir("find-a");
        let d2 = temp_dir("find-b");
        fs::write(d1.join("tool-x"), "").unwrap();
        fs::write(d2.join("tool-x"), "").unwrap();

        let dirs: [&Path; 2] = [&d1, &d2];
        let result = find_path("tool-x", &dirs);
        assert!(
            result.starts_with(d1.to_str().unwrap()),
            "first dir should win, got: {}",
            result
        );
    }

    // ── probe_version ──────────────────────────────────────────────

    #[test]
    fn probe_version_normal() {
        let dir = temp_dir("probe");
        let script_path = write_version_script(&dir, "my-tool", "v1.2.3");
        let result = probe_version("my-tool", script_path.to_str().unwrap());
        assert_eq!(result, Some("v1.2.3".to_string()));
    }

    #[test]
    fn probe_version_blacklisted() {
        // gitk should not spawn a process at all
        let result = probe_version("gitk", "/some/path/gitk");
        assert!(result.is_none(), "blacklisted tool should not be probed");

        let result = probe_version("gvim.exe", "/some/path/gvim.exe");
        assert!(result.is_none(), "gvim.exe should be blacklisted");
    }

    #[test]
    fn probe_version_uses_resolved_path() {
        let dir = temp_dir("probe-resolved");
        let script_path = write_version_script(&dir, "resolved-tool", "v3.0");
        // Pass resolved_path explicitly instead of relying on PATH lookup
        let result = probe_version("resolved-tool", script_path.to_str().unwrap());
        assert_eq!(result, Some("v3.0".to_string()));
    }

    #[test]
    fn probe_version_empty_output() {
        let dir = temp_dir("probe-empty");
        let script = dir.join("silent");
        fs::write(&script, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let result = probe_version("silent", script.to_str().unwrap());
        // No output → no version (tool might be a binary that doesn't support --version)
        assert!(result.is_none());
    }

    #[test]
    fn probe_version_long_line_truncated() {
        let dir = temp_dir("probe-long");
        let long_line = "x".repeat(150);
        let script = write_version_script(&dir, "verbose", &long_line);
        let result = probe_version("verbose", script.to_str().unwrap());
        // Lines longer than 120 chars are skipped
        assert!(result.is_none());
    }

    // ── load_user_tags (parsing logic) ─────────────────────────────

    #[test]
    fn load_user_tags_parses_valid_json() {
        let json = r#"{"git": ["scm", "devops"], "python3": ["python"]}"#;
        let tags: HashMap<String, Vec<String>> = serde_json::from_str(json).unwrap();
        assert_eq!(tags.get("git").unwrap(), &vec!["scm".to_string(), "devops".to_string()]);
        assert_eq!(tags.get("python3").unwrap(), &vec!["python".to_string()]);
    }

    #[test]
    fn load_user_tags_invalid_json_returns_empty() {
        let result: HashMap<String, Vec<String>> = serde_json::from_str("not json").unwrap_or_default();
        assert!(result.is_empty());
    }

    // ── collect_meta ───────────────────────────────────────────────

    #[test]
    fn collect_meta_basic() {
        let tools = vec!["git".to_string(), "python3".to_string(), "curl".to_string()];
        let dirs: &[&Path] = &[];
        let meta = collect_meta(&tools, dirs, false, None);

        assert_eq!(meta.len(), 3);
        assert_eq!(meta["git"].category, Some("scm".to_string()));
        assert_eq!(meta["git"].tags, vec!["scm"]);
        assert!(meta["git"].version.is_none(), "with_versions=false → no version");

        assert_eq!(meta["python3"].category, Some("python".to_string()));
        assert_eq!(meta["curl"].category, Some("network".to_string()));
    }

    #[test]
    fn collect_meta_unknown_tool() {
        let tools = vec!["zzz-nonsense".to_string()];
        let dirs: &[&Path] = &[];
        let meta = collect_meta(&tools, dirs, false, None);

        assert_eq!(meta.len(), 1);
        assert!(meta["zzz-nonsense"].category.is_none());
        assert!(meta["zzz-nonsense"].tags.is_empty());
    }

    #[test]
    fn collect_meta_user_tags_merged() {
        let tools = vec!["git".to_string()];
        let dirs: &[&Path] = &[];
        let mut user_tags = HashMap::new();
        user_tags.insert("git".to_string(), vec!["devops".to_string(), "vcs".to_string()]);

        let meta = collect_meta(&tools, dirs, false, Some(&user_tags));

        // Should have built-in "scm" tag plus user-defined tags
        assert!(meta["git"].tags.contains(&"scm".to_string()));
        assert!(meta["git"].tags.contains(&"devops".to_string()));
        assert!(meta["git"].tags.contains(&"vcs".to_string()));
    }

    #[test]
    fn collect_meta_user_tags_no_duplicate() {
        let tools = vec!["git".to_string()];
        let dirs: &[&Path] = &[];
        let mut user_tags = HashMap::new();
        // User provides "scm" which is already a built-in tag
        user_tags.insert("git".to_string(), vec!["scm".to_string()]);

        let meta = collect_meta(&tools, dirs, false, Some(&user_tags));

        // "scm" should appear only once
        let scm_count = meta["git"].tags.iter().filter(|t| t.as_str() == "scm").count();
        assert_eq!(scm_count, 1, "no duplicate tags");
    }

    #[test]
    fn collect_meta_with_versions_probes() {
        let dir = temp_dir("meta-ver");
        let tool_name = if cfg!(windows) { "ver-probe.bat" } else { "ver-probe" };
        let _ = write_version_script(&dir, &tool_name.trim_end_matches(".bat"), "v4.5.6");

        let tools = vec![tool_name.to_string()];
        let dirs: [&Path; 1] = [&dir];
        let meta = collect_meta(&tools, &dirs, true, None);

        assert!(meta[tool_name].version.is_some(),
            "with_versions=true should probe version: {:?}",
            meta[tool_name].version);
    }
}
