mod metadata;
mod platform;
mod scanner;
mod ui;
mod usage;

use std::collections::HashMap;
use std::process::{self, Command};

/// Parsed CLI arguments, returned by [`parse_args`].
struct ParsedArgs<'a> {
    list_mode: bool,
    json_mode: bool,
    meta_mode: bool,
    project_mode: bool,
    tag_filter: Option<String>,
    select_mode: SelectMode,
    sort_mode: SortMode,
    filter: Option<String>,
    forwarded: Vec<&'a str>,
}

/// Describes how to pick a tool when not in list mode.
enum SelectMode {
    Interactive,
    PickFirst,
    Exact,
    Index(usize),
}

/// Supported sort modes for list output.
#[derive(Clone, Copy, PartialEq)]
enum SortMode {
    Alpha,
    Freq,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // ── Argument parsing ────────────────────────────────────────────
    //
    //   loci -l|--list [...flags...] [filter]
    //   loci [--pick-first | --exact | --index N] [filter...] [-- forwarded-args...]
    //
    // Everything before `--` is a pre-filter string passed to skim.
    // Everything after `--` is forwarded to the selected executable.
    let parsed = parse_args(&args);

    // ── Collect executables ─────────────────────────────────────────
    let scanner = scanner::Scanner::new();
    let executables = if parsed.project_mode {
        let proj = scanner.collect_project();
        if proj.is_empty() {
            eprintln!("loci: no project-local tools found (no package.json / pyvenv.cfg / Cargo.toml detected)");
            process::exit(0);
        }
        proj
    } else {
        scanner.collect()
    };

    if executables.is_empty() {
        eprintln!("loci: no executables found in PATH");
        process::exit(1);
    }

    // ── List mode: print and exit ───────────────────────────────────
    if parsed.list_mode {
        // Tag filter requires metadata (auto-enable meta_mode for tag data).
        let explicit_meta = parsed.meta_mode;
        let meta_mode = parsed.meta_mode || parsed.tag_filter.is_some();
        let user_tags = if meta_mode {
            metadata::load_user_tags()
        } else {
            HashMap::new()
        };

        // 1. Apply name filter
        let name_filtered: Vec<&String> = match &parsed.filter {
            Some(f) => executables
                .iter()
                .filter(|e| fuzzy_match(e, f))
                .collect(),
            None => executables.iter().collect(),
        };

        // 2. Compute metadata once (used for tag filtering and/or JSON output).
        let path_dirs = scanner.path_dirs();
        let meta_cache: Option<HashMap<String, metadata::ToolMeta>> = if meta_mode {
            let owned: Vec<String> = name_filtered.iter().map(|s| (*s).clone()).collect();
            Some(metadata::collect_meta(
                &owned,
                &path_dirs,
                explicit_meta,
                Some(&user_tags),
            ))
        } else {
            None
        };

        // 3. Apply tag filter (semantic, using precomputed meta_cache).
        let filtered: Vec<&String> = if let Some(tag) = &parsed.tag_filter {
            if let Some(ref meta) = meta_cache {
                name_filtered
                    .into_iter()
                    .filter(|e| {
                        meta.get(e.as_str())
                            .map(|m| m.tags.iter().any(|t| t == tag))
                            .unwrap_or(false)
                    })
                    .collect()
            } else {
                debug_assert!(
                    false,
                    "tag_filter={:?} but meta_cache is None — \
                     meta_mode should have been forced true at line 72",
                    tag
                );
                // Release-mode fallback: substring match (debug builds catch the invariant).
                name_filtered
                    .into_iter()
                    .filter(|e| e.to_lowercase().contains(&tag.to_lowercase()))
                    .collect()
            }
        } else {
            name_filtered
        };

        // 4. Apply sorting
        let mut sorted: Vec<String> = filtered.iter().map(|s| (*s).clone()).collect();
        match parsed.sort_mode {
            SortMode::Freq => usage::sort_by_frequency(&mut sorted),
            SortMode::Alpha => usage::sort_alpha(&mut sorted),
        }
        let sorted_refs: Vec<&String> = sorted.iter().collect();

        if parsed.json_mode {
            let names: Vec<&str> = sorted_refs.iter().map(|s| s.as_str()).collect();

            let mut output = serde_json::json!({
                "skill_version": concat!("v", env!("CARGO_PKG_VERSION")),
                "total": names.len(),
                "executables": names,
                "filter": parsed.filter,
            });

            if parsed.tag_filter.is_some() {
                output["tag_filter"] = serde_json::json!(parsed.tag_filter);
            }

            if parsed.project_mode {
                output["project"] = serde_json::json!(true);
            }

            // Metadata with tags (version probing only when --meta explicitly given).
            if let Some(ref meta) = meta_cache {
                // Filter to only include items in the final sorted list.
                let sorted_names: Vec<&str> =
                    sorted_refs.iter().map(|s| s.as_str()).collect();
                let sorted_meta: HashMap<String, metadata::ToolMeta> = meta
                    .iter()
                    .filter(|(name, _)| sorted_names.contains(&name.as_str()))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                output["meta"] =
                    serde_json::to_value(&sorted_meta).unwrap_or(serde_json::Value::Null);
            }

            match serde_json::to_string_pretty(&output) {
                Ok(json) => println!("{}", json),
                Err(e) => {
                    eprintln!(
                        "loci: warning: JSON output formatting failed, \
                         falling back to compact format: {}",
                        e
                    );
                    if let Ok(compact) = serde_json::to_string(&output) {
                        println!("{}", compact);
                    }
                }
            }
        } else {
            for exe in sorted_refs {
                println!("{}", exe);
            }
        }
        process::exit(0);
    }

    // ── Interactive / programmatic selection ────────────────────────
    match parsed.select_mode {
        SelectMode::Interactive => {
            let selected = ui::select_executable(&executables, parsed.filter.as_deref());
            if let Some(tool) = selected {
                usage::record_usage(&tool);
                launch(&tool, &parsed.forwarded);
            }
            process::exit(0); // user cancelled
        }
        SelectMode::PickFirst => {
            let filtered: Vec<&String> = match &parsed.filter {
                Some(f) => executables.iter().filter(|e| fuzzy_match(e, f)).collect(),
                None => executables.iter().collect(),
            };
            if let Some(tool) = filtered.first() {
                usage::record_usage(tool);
                launch(tool, &parsed.forwarded);
            }
            eprintln!(
                "loci: no matching tool for '{}'",
                parsed.filter.as_deref().unwrap_or("")
            );
            process::exit(1);
        }
        SelectMode::Exact => {
            if let Some(f) = &parsed.filter {
                if let Some(tool) = executables.iter().find(|e| *e == f) {
                    usage::record_usage(tool);
                    launch(tool, &parsed.forwarded);
                }
            }
            eprintln!(
                "loci: exact match not found for '{}'",
                parsed.filter.as_deref().unwrap_or("")
            );
            process::exit(1);
        }
        SelectMode::Index(idx) => {
            let filtered: Vec<&String> = match &parsed.filter {
                Some(f) => executables.iter().filter(|e| fuzzy_match(e, f)).collect(),
                None => executables.iter().collect(),
            };
            if idx < filtered.len() {
                usage::record_usage(filtered[idx]);
                launch(filtered[idx], &parsed.forwarded);
            }
            eprintln!(
                "loci: index {} out of range (0..{}) for '{}'",
                idx,
                filtered.len(),
                parsed.filter.as_deref().unwrap_or("")
            );
            process::exit(1);
        }
    }
}

/// Parse CLI arguments into a [`ParsedArgs`] struct.
fn parse_args<'a>(args: &'a [String]) -> ParsedArgs<'a> {
    if args.len() <= 1 {
        return ParsedArgs {
            list_mode: false,
            json_mode: false,
            meta_mode: false,
            project_mode: false,
            tag_filter: None,
            select_mode: SelectMode::Interactive,
            sort_mode: SortMode::Alpha,
            filter: None,
            forwarded: vec![],
        };
    }

    // Scan for simple flags anywhere in args (position-independent)
    let json_mode = args[1..].iter().any(|a| a == "--json");
    let meta_mode = args[1..].iter().any(|a| a == "--meta");
    let project_mode = args[1..].iter().any(|a| a == "--project");
    let pick_first = args[1..].iter().any(|a| a == "--pick-first");
    let exact_mode = args[1..].iter().any(|a| a == "--exact");
    let sort_flag_seen = args[1..].iter().any(|a| a == "--sort");
    let tag_flag_seen = args[1..].iter().any(|a| a == "--tag");

    // Extract --sort <mode>
    let sort_mode: SortMode = {
        let mut sm = SortMode::Alpha;
        let mut i = 1;
        while i < args.len() {
            if args[i] == "--sort" && i + 1 < args.len() {
                sm = match args[i + 1].as_str() {
                    "freq" | "frequency" => SortMode::Freq,
                    _ => SortMode::Alpha,
                };
                break;
            }
            i += 1;
        }
        sm
    };

    let mut index_flag_seen = false;
    let select_mode: SelectMode = if pick_first {
        SelectMode::PickFirst
    } else if exact_mode {
        SelectMode::Exact
    } else {
        let mut idx = None;
        let mut i = 1;
        while i < args.len() {
            if args[i] == "--index" {
                index_flag_seen = true;
                if i + 1 < args.len() {
                    idx = args[i + 1].parse::<usize>().ok();
                    break;
                }
            }
            i += 1;
        }
        match idx {
            Some(n) => SelectMode::Index(n),
            None => SelectMode::Interactive,
        }
    };

    // Extract --tag <name> (value follows the flag)
    let tag_filter: Option<String> = {
        let mut tag = None;
        let mut i = 1;
        while i < args.len() {
            if args[i] == "--tag" && i + 1 < args.len() {
                tag = Some(args[i + 1].clone());
                break;
            }
            i += 1;
        }
        tag
    };

    // Check for -l / --list flag (must be in first argument position)
    let list_mode = args[1] == "-l" || args[1] == "--list";
    let start = if list_mode { 2 } else { 1 };

    if start >= args.len() {
        return ParsedArgs {
            list_mode,
            json_mode,
            meta_mode,
            project_mode,
            tag_filter,
            select_mode,
            sort_mode,
            filter: None,
            forwarded: vec![],
        };
    }

    // Build a set of tokens to skip (flags that should not become filter strings).
    let mut skip_flags: Vec<&str> = vec![
        "--json",
        "--meta",
        "--project",
        "--pick-first",
        "--exact",
        "--sort",
    ];
    if tag_filter.is_some() {
        skip_flags.push("--tag");
    }
    if let SelectMode::Index(_) = select_mode {
        skip_flags.push("--index");
    }

    let rest: Vec<&str> = {
        let mut collected = Vec::new();
        let mut skip_next = false;
        for a in args[start..].iter().map(String::as_str) {
            if skip_next {
                skip_next = false;
                continue;
            }
            if skip_flags.contains(&a) {
                // --tag, --index and --sort consume the next arg too
                skip_next = a == "--tag" || a == "--index" || a == "--sort";
                continue;
            }
            collected.push(a);
        }
        collected
    };

    let dashdash = rest.iter().position(|a| *a == "--");

    let filter = match dashdash {
        Some(0) => None,
        Some(pos) => Some(rest[..pos].join(" ")),
        None if !rest.is_empty() => Some(rest.join(" ")),
        None => None,
    };

    let forwarded: Vec<&'a str> = dashdash
        .map(|pos| rest[pos + 1..].to_vec())
        .unwrap_or_default();

    // ── Value-flag validation warnings ──────────────────────────────
    if tag_flag_seen && tag_filter.is_none() {
        eprintln!(
            "loci: warning: --tag requires a name argument \
             (e.g. --tag scm), ignoring flag"
        );
    }
    if index_flag_seen && matches!(select_mode, SelectMode::Interactive) {
        eprintln!(
            "loci: warning: --index requires a number argument, ignoring flag"
        );
    }
    if sort_flag_seen && sort_mode == SortMode::Alpha
        && args.last().map(|s| s.as_str()) == Some("--sort")
    {
        eprintln!(
            "loci: warning: --sort requires a mode argument \
             (e.g. --sort freq), ignoring flag"
        );
    }

    ParsedArgs {
        list_mode,
        json_mode,
        meta_mode,
        project_mode,
        tag_filter,
        select_mode,
        sort_mode,
        filter,
        forwarded,
    }
}

/// Simple case-insensitive substring match for `loci -l <filter>`.
fn fuzzy_match(name: &str, filter: &str) -> bool {
    name.to_lowercase().contains(&filter.to_lowercase())
}

/// Replace the current process with the selected tool (Unix `exec`)
/// or spawn-and-wait (Windows fallback).  stdin/stdout/stderr are
/// inherited automatically.
fn launch(tool: &str, args: &[&str]) -> ! {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = Command::new(tool).args(args).exec();
        // exec() only returns on error
        eprintln!("loci: failed to execute '{}': {}", tool, err);
        process::exit(1);
    }

    #[cfg(not(unix))]
    {
        let status = Command::new(tool)
            .args(args)
            .status()
            .unwrap_or_else(|e| {
                eprintln!("loci: failed to execute '{}': {}", tool, e);
                process::exit(1);
            });
        match status.code() {
            Some(code) => process::exit(code),
            None => {
                eprintln!("loci: child process was terminated abnormally (exit code unavailable)");
                process::exit(1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests (only compile when testing)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    // ── Default (no args) ──────────────────────────────────────────

    #[test]
    fn parse_args_default() {
        let a = vec!["loci".to_string()];
        let p = parse_args(&a);
        assert!(!p.list_mode, "list_mode");
        assert!(!p.json_mode, "json_mode");
        assert!(!p.meta_mode, "meta_mode");
        assert!(!p.project_mode, "project_mode");
        assert!(p.tag_filter.is_none(), "tag_filter");
        assert!(matches!(p.select_mode, SelectMode::Interactive), "select_mode");
        assert!(matches!(p.sort_mode, SortMode::Alpha), "sort_mode");
        assert!(p.filter.is_none(), "filter");
        assert!(p.forwarded.is_empty(), "forwarded");
    }

    // ── List mode basics ───────────────────────────────────────────

    #[test]
    fn parse_args_list_short() {
        let a = vec!["loci".to_string(), "-l".to_string()];
        let p = parse_args(&a);
        assert!(p.list_mode);
        assert!(!p.json_mode);
    }

    #[test]
    fn parse_args_list_long() {
        let a = vec!["loci".to_string(), "--list".to_string()];
        let p = parse_args(&a);
        assert!(p.list_mode);
    }

    #[test]
    fn parse_args_list_with_filter() {
        let a = vec!["loci".to_string(), "-l".to_string(), "git".to_string()];
        let p = parse_args(&a);
        assert!(p.list_mode);
        assert_eq!(p.filter, Some("git".to_string()));
    }

    #[test]
    fn parse_args_list_json() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--json".to_string()];
        let p = parse_args(&a);
        assert!(p.json_mode);
    }

    #[test]
    fn parse_args_list_json_filter() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--json".to_string(), "python".to_string()];
        let p = parse_args(&a);
        assert!(p.json_mode);
        assert_eq!(p.filter, Some("python".to_string()));
    }

    #[test]
    fn parse_args_list_meta() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--meta".to_string()];
        let p = parse_args(&a);
        assert!(p.meta_mode);
        assert!(p.list_mode);
    }

    #[test]
    fn parse_args_list_json_meta() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--json".to_string(), "--meta".to_string()];
        let p = parse_args(&a);
        assert!(p.json_mode);
        assert!(p.meta_mode);
    }

    #[test]
    fn parse_args_list_meta_tag() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--meta".to_string(), "--tag".to_string(), "python".to_string()];
        let p = parse_args(&a);
        assert!(p.meta_mode);
        assert_eq!(p.tag_filter, Some("python".to_string()));
    }

    // ── Tag ────────────────────────────────────────────────────────

    #[test]
    fn parse_args_tag_filter() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--tag".to_string(), "scm".to_string()];
        let p = parse_args(&a);
        assert_eq!(p.tag_filter, Some("scm".to_string()));
    }

    // ── Project mode ───────────────────────────────────────────────

    #[test]
    fn parse_args_project() {
        let a = vec!["loci".to_string(), "--project".to_string()];
        let p = parse_args(&a);
        assert!(p.project_mode);
    }

    #[test]
    fn parse_args_project_json() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--json".to_string(), "--project".to_string()];
        let p = parse_args(&a);
        assert!(p.project_mode);
        assert!(p.json_mode);
    }

    // ── Select modes ───────────────────────────────────────────────

    #[test]
    fn parse_args_exact() {
        let a = vec!["loci".to_string(), "--exact".to_string(), "python.exe".to_string()];
        let p = parse_args(&a);
        assert!(matches!(p.select_mode, SelectMode::Exact));
        assert_eq!(p.filter, Some("python.exe".to_string()));
    }

    #[test]
    fn parse_args_pick_first() {
        let a = vec!["loci".to_string(), "--pick-first".to_string(), "cargo".to_string()];
        let p = parse_args(&a);
        assert!(matches!(p.select_mode, SelectMode::PickFirst));
        assert_eq!(p.filter, Some("cargo".to_string()));
    }

    #[test]
    fn parse_args_index_zero() {
        let a = vec!["loci".to_string(), "--index".to_string(), "0".to_string(), "git".to_string()];
        let p = parse_args(&a);
        assert!(matches!(p.select_mode, SelectMode::Index(0)));
        assert_eq!(p.filter, Some("git".to_string()));
    }

    #[test]
    fn parse_args_index_large() {
        let a = vec!["loci".to_string(), "--index".to_string(), "42".to_string()];
        let p = parse_args(&a);
        assert!(matches!(p.select_mode, SelectMode::Index(42)));
    }

    // ── Sort ───────────────────────────────────────────────────────

    #[test]
    fn parse_args_sort_alpha_default() {
        let a = vec!["loci".to_string(), "-l".to_string()];
        let p = parse_args(&a);
        assert!(matches!(p.sort_mode, SortMode::Alpha));
    }

    #[test]
    fn parse_args_sort_freq() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--sort".to_string(), "freq".to_string()];
        let p = parse_args(&a);
        assert!(matches!(p.sort_mode, SortMode::Freq));
    }

    #[test]
    fn parse_args_sort_frequency_alias() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--sort".to_string(), "frequency".to_string()];
        let p = parse_args(&a);
        assert!(matches!(p.sort_mode, SortMode::Freq));
    }

    #[test]
    fn parse_args_sort_with_keyword() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--sort".to_string(), "freq".to_string(), "rust".to_string()];
        let p = parse_args(&a);
        assert!(matches!(p.sort_mode, SortMode::Freq));
        assert_eq!(p.filter, Some("rust".to_string()));
    }

    // ── Argument passthrough ───────────────────────────────────────

    #[test]
    fn parse_args_forwarded() {
        let a = vec!["loci".to_string(), "git".to_string(), "--".to_string(), "log".to_string(), "--oneline".to_string()];
        let p = parse_args(&a);
        assert_eq!(p.filter, Some("git".to_string()));
        assert_eq!(p.forwarded, vec!["log", "--oneline"]);
    }

    #[test]
    fn parse_args_forwarded_no_filter() {
        let a = vec!["loci".to_string(), "--".to_string(), "log".to_string(), "--oneline".to_string()];
        let p = parse_args(&a);
        assert!(p.filter.is_none());
        assert_eq!(p.forwarded, vec!["log", "--oneline"]);
    }

    #[test]
    fn parse_args_forwarded_empty() {
        let a = vec!["loci".to_string(), "git".to_string(), "--".to_string()];
        let p = parse_args(&a);
        assert_eq!(p.filter, Some("git".to_string()));
        assert!(p.forwarded.is_empty());
    }

    #[test]
    fn parse_args_dashdash_only() {
        let a = vec!["loci".to_string(), "--".to_string()];
        let p = parse_args(&a);
        assert!(p.filter.is_none());
        assert!(p.forwarded.is_empty());
    }

    // ── Multi-word filter ──────────────────────────────────────────

    #[test]
    fn parse_args_multi_word_filter() {
        let a = vec!["loci".to_string(), "-l".to_string(), "multi".to_string(), "word".to_string(), "filter".to_string()];
        let p = parse_args(&a);
        assert_eq!(p.filter, Some("multi word filter".to_string()));
    }

    // ── Flag position independence (`-l`/`--list` must be at args[1],
    //     but other flags like --json are found anywhere) ────────────

    #[test]
    fn parse_args_json_list_swapped_still_finds_flags() {
        // --json before -l → json_mode found (scanning all args), list_mode too
        let a = vec!["loci".to_string(), "-l".to_string(), "--json".to_string()];
        let p = parse_args(&a);
        assert!(p.json_mode);
        assert!(p.list_mode);
    }

    #[test]
    fn parse_args_json_after_filter() {
        // --json after the filter keyword
        let a = vec!["loci".to_string(), "-l".to_string(), "git".to_string(), "--json".to_string()];
        let p = parse_args(&a);
        assert!(p.json_mode);
        assert_eq!(p.filter, Some("git".to_string()));
    }

    // ── Missing value warnings ─────────────────────────────────────

    #[test]
    fn parse_args_missing_tag_value() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--tag".to_string()];
        let p = parse_args(&a);
        assert!(p.tag_filter.is_none());
    }

    #[test]
    fn parse_args_missing_index_value() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--index".to_string()];
        let p = parse_args(&a);
        assert!(matches!(p.select_mode, SelectMode::Interactive));
    }

    #[test]
    fn parse_args_missing_sort_value() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--sort".to_string()];
        let p = parse_args(&a);
        assert!(matches!(p.sort_mode, SortMode::Alpha));
    }

    // ── Combinations ───────────────────────────────────────────────

    #[test]
    fn parse_args_tag_json_keyword() {
        let a = vec![
            "loci".to_string(), "-l".to_string(), "--json".to_string(),
            "--tag".to_string(), "scm".to_string(), "git".to_string(),
        ];
        let p = parse_args(&a);
        assert_eq!(p.tag_filter, Some("scm".to_string()));
        assert_eq!(p.filter, Some("git".to_string()));
        assert!(p.json_mode);
    }

    #[test]
    fn parse_args_sort_freq_json_meta() {
        let a = vec![
            "loci".to_string(), "-l".to_string(), "--sort".to_string(),
            "freq".to_string(), "--json".to_string(), "--meta".to_string(),
        ];
        let p = parse_args(&a);
        assert!(matches!(p.sort_mode, SortMode::Freq));
        assert!(p.json_mode);
        assert!(p.meta_mode);
    }

    #[test]
    fn parse_args_all_flags() {
        let a = vec![
            "loci".to_string(), "-l".to_string(), "--json".to_string(),
            "--meta".to_string(), "--tag".to_string(), "python".to_string(),
            "--sort".to_string(), "freq".to_string(),
        ];
        let p = parse_args(&a);
        assert!(p.list_mode);
        assert!(p.json_mode);
        assert!(p.meta_mode);
        assert_eq!(p.tag_filter, Some("python".to_string()));
        assert!(matches!(p.sort_mode, SortMode::Freq));
    }

    #[test]
    fn parse_args_with_project() {
        let a = vec![
            "loci".to_string(), "-l".to_string(), "--json".to_string(),
            "--project".to_string(), "python".to_string(),
        ];
        let p = parse_args(&a);
        assert!(p.project_mode);
        assert!(p.json_mode);
        assert_eq!(p.filter, Some("python".to_string()));
    }

    // ── fuzzy_match ────────────────────────────────────────────────

    #[test]
    fn fuzzy_match_exact() {
        assert!(fuzzy_match("git", "git"));
    }

    #[test]
    fn fuzzy_match_case_insensitive() {
        assert!(fuzzy_match("Git", "git"));
        assert!(fuzzy_match("git", "GIT"));
    }

    #[test]
    fn fuzzy_match_substring() {
        assert!(fuzzy_match("git-lfs", "git"));
        assert!(fuzzy_match("python3", "thon"));
    }

    #[test]
    fn fuzzy_match_no_match() {
        assert!(!fuzzy_match("python3", "ruby"));
    }

    #[test]
    fn fuzzy_match_empty_filter() {
        assert!(fuzzy_match("abc", ""));
        assert!(fuzzy_match("", ""));
    }

    #[test]
    fn fuzzy_match_empty_name_nonempty_filter() {
        assert!(!fuzzy_match("", "x"));
    }
}
