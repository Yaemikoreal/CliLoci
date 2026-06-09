//! Integration tests for the `loci` binary.
//!
//! These tests compile against the real binary, running it in subprocesses
//! with controlled PATH environments to verify CLI behavior end-to-end.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Path to the compiled `loci` binary (set by Cargo's test harness).
const LOCI_BIN: &str = env!("CARGO_BIN_EXE_loci");

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run `loci` with `args` and return the Output.
fn loci(args: &[&str]) -> Output {
    Command::new(LOCI_BIN)
        .args(args)
        .output()
        .expect("failed to run loci")
}

/// Run `loci` with `args` under a custom PATH and return the Output.
fn loci_with_path(args: &[&str], path: &str) -> Output {
    Command::new(LOCI_BIN)
        .env("PATH", path)
        .args(args)
        .output()
        .expect("failed to run loci")
}

/// Create a file that will be detected as executable by loci.
/// On Unix: sets the executable permission bit (0o755).
/// On Windows: appends .exe extension so PATHEXT matches it.
fn create_executable(dir: &Path, name: &str) -> PathBuf {
    let path = if cfg!(windows) {
        let mut p = dir.join(name);
        // Ensure .exe extension for PATHEXT matching on Windows
        if p.extension().is_none() {
            p.set_extension("exe");
        }
        p
    } else {
        dir.join(name)
    };
    std::fs::write(&path, "").expect("write test tool file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("set executable bit");
    }
    path
}

/// Return the display name that loci will use for a tool on this platform.
/// On Windows, PATHEXT extensions (.exe) are included in the filename.
fn tool_display_name(name: &str) -> String {
    if cfg!(windows) {
        let has_ext = Path::new(name).extension().is_some();
        if has_ext { name.to_string() } else { format!("{}.exe", name) }
    } else {
        name.to_string()
    }
}

/// Create a unique temp directory for a test case.
fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "loci-integration-{}-{}", label, std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Assert that `output` is successful (exit code 0).
#[track_caller]
fn assert_ok(output: &Output) {
    assert!(
        output.status.success(),
        "expected success, got exit={:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Assert that `output` exited with a specific code.
#[track_caller]
fn assert_exit_code(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "expected exit code {}, got {:?}\nstderr: {}",
        code,
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Parse JSON from stdout and return the `executables` array.
fn parse_json_executables(stdout: &[u8]) -> Vec<String> {
    let val: serde_json::Value =
        serde_json::from_slice(stdout).expect("valid JSON output");
    val["executables"]
        .as_array()
        .expect("executables array")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

/// Parse full JSON output from stdout.
fn parse_json(stdout: &[u8]) -> serde_json::Value {
    serde_json::from_slice(stdout).expect("valid JSON output")
}

// ============================================================================
// Tests
// ============================================================================

// ── Listing ────────────────────────────────────────────────────────────────

#[test]
fn list_mode_text_has_output() {
    let output = loci(&["-l"]);
    assert_ok(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "stdout should not be empty");
    for line in stdout.lines().take(5) {
        assert!(!line.is_empty(), "lines should not be empty");
        assert!(!line.starts_with(' '), "no leading whitespace: '{:?}'", line);
    }
}

#[test]
fn list_json_has_required_fields() {
    let output = loci(&["-l", "--json"]);
    assert_ok(&output);

    let json = parse_json(&output.stdout);
    assert!(json["skill_version"].is_string(), "skill_version must be a string");
    assert!(json["total"].is_u64(), "total must be a number");
    assert!(json["executables"].is_array(), "executables must be an array");
    assert_eq!(
        json["total"].as_u64().unwrap() as usize,
        json["executables"].as_array().unwrap().len(),
        "total must match executables length",
    );
}

#[test]
fn list_json_filtered_returns_only_matches() {
    let path = std::env::var("PATH").unwrap_or_default();
    let output = loci_with_path(&["-l", "--json", "cargo"], &path);
    assert_ok(&output);

    let tools = parse_json_executables(&output.stdout);
    for t in &tools {
        assert!(
            t.to_lowercase().contains("cargo"),
            "filter 'cargo' matched '{}'",
            t,
        );
    }
}

#[test]
fn list_json_nonexistent_filter_returns_empty() {
    let path = std::env::var("PATH").unwrap_or_default();
    let output = loci_with_path(&["-l", "--json", "XYZZYX_NONEXISTENT"], &path);
    assert_ok(&output);

    let json = parse_json(&output.stdout);
    assert_eq!(json["total"].as_u64().unwrap(), 0);
    assert!(json["executables"].as_array().unwrap().is_empty());
}

// ── JSON output fields ────────────────────────────────────────────────────

#[test]
fn list_json_includes_skill_version() {
    let output = loci(&["-l", "--json"]);
    assert_ok(&output);
    let json = parse_json(&output.stdout);
    let ver = json["skill_version"].as_str().unwrap();
    assert!(ver.starts_with('v'), "skill_version should start with 'v': {}", ver);
}

#[test]
fn list_json_includes_filter_key() {
    let output = loci(&["-l", "--json", "git"]);
    assert_ok(&output);
    let json = parse_json(&output.stdout);
    assert_eq!(json["filter"], "git");
}

#[test]
fn list_json_project_flag_appears() {
    let output = loci(&["-l", "--json", "--project"]);
    // project may return 0 tools if not in a project dir, that's fine
    let json = parse_json(&output.stdout);
    assert_eq!(json["project"], true, "project flag should be in JSON");
}

// ── Meta and tags ─────────────────────────────────────────────────────────

#[test]
fn list_json_with_meta_has_category() {
    let path = std::env::var("PATH").unwrap_or_default();
    let output = loci_with_path(&["-l", "--json", "--meta", "git"], &path);
    assert_ok(&output);

    let json = parse_json(&output.stdout);
    assert!(
        json["meta"].is_object(),
        "meta should be present when --meta is given"
    );
    for (_tool, info) in json["meta"].as_object().unwrap() {
        assert!(info.get("category").is_some(), "each meta entry should have a category");
        assert!(info.get("tags").is_some(), "each meta entry should have tags");
        assert!(info.get("path").is_some(), "each meta entry should have a path");
    }
}

#[test]
fn list_json_tag_filter_appears() {
    let path = std::env::var("PATH").unwrap_or_default();
    let output = loci_with_path(&["-l", "--json", "--tag", "scm"], &path);
    assert_ok(&output);

    let json = parse_json(&output.stdout);
    assert_eq!(json["tag_filter"], "scm");
    assert!(
        json["meta"].is_object(),
        "meta should be present with --tag"
    );
}

#[test]
fn list_json_tag_scm_returns_only_scm_tools() {
    let path = std::env::var("PATH").unwrap_or_default();
    let output = loci_with_path(&["-l", "--json", "--tag", "scm"], &path);
    assert_ok(&output);

    let json = parse_json(&output.stdout);
    let meta = json["meta"].as_object().unwrap();
    for (_tool, info) in meta {
        let tags = info["tags"].as_array().unwrap();
        let has_scm = tags.iter().any(|t| t.as_str() == Some("scm"));
        assert!(has_scm, "tool with tag scm should have scm in tags, got: {:?}", tags);
    }
}

// ── Custom PATH ───────────────────────────────────────────────────────────

#[test]
fn custom_path_lists_only_those_tools() {
    let dir = temp_dir("custom-path");
    let name_a = tool_display_name("my-tool-a");
    let name_b = tool_display_name("my-tool-b");
    create_executable(&dir, "my-tool-a");
    create_executable(&dir, "my-tool-b");

    let output = loci_with_path(&["-l", "--json"], dir.to_str().unwrap());
    assert_ok(&output);

    let tools = parse_json_executables(&output.stdout);
    assert!(
        tools.contains(&name_a),
        "should contain {}, got: {:?}",
        name_a, tools,
    );
    assert!(
        tools.contains(&name_b),
        "should contain {}, got: {:?}",
        name_b, tools,
    );
}

#[test]
fn custom_path_empty_exits_with_error() {
    let dir = temp_dir("empty-path");
    let output = loci_with_path(&["-l", "--json"], dir.to_str().unwrap());
    // loci exits with 1 when no executables found
    assert_exit_code(&output, 1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no executables found"),
        "stderr should mention no executables: {}",
        stderr,
    );
}

// ── Programmatic selection (exit codes) ───────────────────────────────────

#[test]
fn exact_match_nonexistent_errors() {
    let path = std::env::var("PATH").unwrap_or_default();
    let output = loci_with_path(&["--exact", "NONEXISTENT_MAGIC"], &path);
    assert_exit_code(&output, 1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("exact match not found"),
        "stderr should mention 'exact match not found': {}",
        stderr,
    );
}

#[test]
fn pick_first_nonexistent_errors() {
    let path = std::env::var("PATH").unwrap_or_default();
    let output = loci_with_path(&["--pick-first", "XYZZYX_NOPE"], &path);
    assert_exit_code(&output, 1);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no matching tool"),
        "stderr should mention 'no matching tool'"
    );
}

#[test]
fn index_out_of_range_errors() {
    // NOTE: --index without -l, in non-list mode (select mode)
    let path = std::env::var("PATH").unwrap_or_default();
    let output = loci_with_path(&["--index", "9999", "loci"], &path);
    assert_exit_code(&output, 1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("out of range") || stderr.contains("no matching tool"),
        "stderr should mention index error: {}",
        stderr,
    );
}

// ── LOCI_PATH_EXTRA ───────────────────────────────────────────────────────

#[test]
fn loci_path_extra_appends_to_path() {
    let dir = temp_dir("extra");
    create_executable(&dir, "extra-tool");
    let name = tool_display_name("extra-tool");

    let base_path = std::env::var("PATH").unwrap_or_default();

    let output = Command::new(LOCI_BIN)
        .env("PATH", &base_path)
        .env("LOCI_PATH_EXTRA", dir.to_str().unwrap())
        .args(&["-l", "--json"])
        .output()
        .expect("failed to run loci with LOCI_PATH_EXTRA");
    assert_ok(&output);

    let tools = parse_json_executables(&output.stdout);
    assert!(
        tools.iter().any(|t| *t == name),
        "should contain '{}', got: {:?}",
        name,
        tools.iter().filter(|t| t.contains("extra")).collect::<Vec<_>>(),
    );
}

// ── Project mode ──────────────────────────────────────────────────────────

#[test]
fn project_mode_no_project_returns_empty_message() {
    let empty_dir = temp_dir("no-project");
    let output = Command::new(LOCI_BIN)
        .current_dir(&empty_dir)
        .args(&["-l", "--project"])
        .output()
        .expect("failed to run loci");
    // loci exits 0 when no project tools found, prints message on stderr
    assert_ok(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no project-local tools found"),
        "stderr should mention no project tools: {}",
        stderr,
    );
}

#[test]
fn project_mode_finds_rust_tools() {
    let proj_dir = temp_dir("rust-proj");
    std::fs::write(proj_dir.join("Cargo.toml"), "").unwrap();
    let debug_dir = proj_dir.join("target").join("debug");
    std::fs::create_dir_all(&debug_dir).unwrap();
    create_executable(&debug_dir, "my-test-crate");
    let name = tool_display_name("my-test-crate");

    let output = Command::new(LOCI_BIN)
        .current_dir(&proj_dir)
        .args(&["-l", "--json", "--project"])
        .output()
        .expect("failed to run loci");
    assert_ok(&output);

    let tools = parse_json_executables(&output.stdout);
    assert!(
        tools.contains(&name),
        "project tools should contain '{}', got: {:?}",
        name,
        tools,
    );
}

// ── Sort mode ─────────────────────────────────────────────────────────────

#[test]
fn sort_mode_alpha_is_default() {
    let dir = temp_dir("sort-alpha");
    create_executable(&dir, "c-tool");
    create_executable(&dir, "a-tool");
    create_executable(&dir, "b-tool");
    let name_a = tool_display_name("a-tool");
    let name_b = tool_display_name("b-tool");
    let name_c = tool_display_name("c-tool");

    let output = loci_with_path(&["-l", "--json", "--sort", "alpha"], dir.to_str().unwrap());
    assert_ok(&output);

    let tools = parse_json_executables(&output.stdout);
    assert_eq!(tools, vec![name_a, name_b, name_c], "alpha sort");
}

// ── Stderr warnings ───────────────────────────────────────────────────────

#[test]
fn missing_tag_value_warns() {
    let output = loci(&["-l", "--tag"]);
    assert_ok(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning"), "should warn about missing --tag value");
}

#[test]
fn missing_index_value_warns() {
    let output = loci(&["-l", "--index"]);
    assert_ok(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning"), "should warn about missing --index value");
}

#[test]
fn missing_sort_value_warns() {
    let output = loci(&["-l", "--sort"]);
    assert_ok(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning"), "should warn about missing --sort value");
}

// ── JSON filter field ─────────────────────────────────────────────────────

#[test]
fn json_filter_field_is_null_without_filter() {
    let output = loci(&["-l", "--json"]);
    assert_ok(&output);
    let json = parse_json(&output.stdout);
    assert!(json["filter"].is_null(), "filter should be null without --filter");
}

#[test]
fn json_filter_field_is_string_with_filter() {
    let output = loci(&["-l", "--json", "python"]);
    assert_ok(&output);
    let json = parse_json(&output.stdout);
    assert_eq!(json["filter"], "python");
}

#[test]
fn json_meta_not_present_without_flag() {
    let output = loci(&["-l", "--json"]);
    assert_ok(&output);
    let json = parse_json(&output.stdout);
    assert!(json.get("meta").is_none(), "meta should NOT be present without --meta");
}

#[test]
fn json_tag_filter_not_present_without_flag() {
    let output = loci(&["-l", "--json"]);
    assert_ok(&output);
    let json = parse_json(&output.stdout);
    assert!(json.get("tag_filter").is_none(), "tag_filter should NOT be present without --tag");
}
