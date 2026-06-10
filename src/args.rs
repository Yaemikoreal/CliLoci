/// CLI argument parsing for loci.
///
/// Extracted from `main.rs` to keep argument handling
/// testable and independently verifiable.

/// Parsed CLI arguments, returned by [`parse_args`].
#[derive(Debug, Clone)]
pub struct ParsedArgs<'a> {
    pub list_mode: bool,
    pub json_mode: bool,
    pub meta_mode: bool,
    pub project_mode: bool,
    pub tag_filter: Option<String>,
    pub count_mode: bool,
    pub limit: Option<usize>,
    pub select_mode: SelectMode,
    pub sort_mode: SortMode,
    pub filter: Option<String>,
    pub forwarded: Vec<&'a str>,
}

/// Describes how to pick a tool when not in list mode.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectMode {
    Interactive,
    PickFirst,
    Exact,
    Index(usize),
}

/// Supported sort modes for list output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortMode {
    Alpha,
    Freq,
}

/// Parse CLI arguments into a [`ParsedArgs`] struct.
pub fn parse_args<'a>(args: &'a [String]) -> ParsedArgs<'a> {
    if args.len() <= 1 {
        return ParsedArgs {
            list_mode: false,
            json_mode: false,
            meta_mode: false,
            project_mode: false,
            tag_filter: None,
            count_mode: false,
            limit: None,
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
    let count_mode = args[1..].iter().any(|a| a == "--count");
    let pick_first = args[1..].iter().any(|a| a == "--pick-first");
    let exact_mode = args[1..].iter().any(|a| a == "--exact");
    let sort_flag_seen = args[1..].iter().any(|a| a == "--sort");
    let tag_flag_seen = args[1..].iter().any(|a| a == "--tag");
    let top_seen = args[1..].iter().any(|a| a == "--top");

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
    // --top implies --sort freq (overrides explicit --sort alpha)
    let sort_mode = if top_seen { SortMode::Freq } else { sort_mode };

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

    // Extract --limit <N> or --top <N>
    let limit: Option<usize> = {
        let mut lim = None;
        let mut i = 1;
        while i < args.len() {
            if (args[i] == "--limit" || args[i] == "--top") && i + 1 < args.len() {
                lim = args[i + 1].parse::<usize>().ok();
                break;
            }
            i += 1;
        }
        lim
    };

    if start >= args.len() {
        return ParsedArgs {
            list_mode,
            json_mode,
            meta_mode,
            project_mode,
            tag_filter,
            count_mode,
            limit,
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
        "--count",
        "--pick-first",
        "--exact",
        "--sort",
        "--limit",
        "--top",
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
                // --tag, --index, --sort, --limit and --top consume the next arg too
                skip_next = a == "--tag" || a == "--index" || a == "--sort"
                    || a == "--limit" || a == "--top";
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
        count_mode,
        limit,
        select_mode,
        sort_mode,
        filter,
        forwarded,
    }
}

/// Simple case-insensitive substring match for `loci -l <filter>`.
pub fn fuzzy_match(name: &str, filter: &str) -> bool {
    name.to_lowercase().contains(&filter.to_lowercase())
}

// ---------------------------------------------------------------------------
// Tests
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

    // ── Count / Limit / Top ────────────────────────────────────────

    #[test]
    fn parse_args_count_flag() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--count".to_string()];
        let p = parse_args(&a);
        assert!(p.count_mode);
        assert!(p.list_mode);
    }

    #[test]
    fn parse_args_count_with_json() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--json".to_string(), "--count".to_string()];
        let p = parse_args(&a);
        assert!(p.count_mode);
        assert!(p.json_mode);
    }

    #[test]
    fn parse_args_limit() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--limit".to_string(), "5".to_string()];
        let p = parse_args(&a);
        assert_eq!(p.limit, Some(5));
    }

    #[test]
    fn parse_args_limit_with_filter() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--limit".to_string(), "10".to_string(), "git".to_string()];
        let p = parse_args(&a);
        assert_eq!(p.limit, Some(10));
        assert_eq!(p.filter, Some("git".to_string()));
    }

    #[test]
    fn parse_args_limit_invalid_value_falls_back() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--limit".to_string(), "abc".to_string()];
        let p = parse_args(&a);
        assert!(p.limit.is_none());
    }

    #[test]
    fn parse_args_top_implies_sort_freq() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--top".to_string(), "5".to_string()];
        let p = parse_args(&a);
        assert_eq!(p.limit, Some(5));
        assert!(matches!(p.sort_mode, SortMode::Freq));
    }

    #[test]
    fn parse_args_top_with_json() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--json".to_string(), "--top".to_string(), "10".to_string()];
        let p = parse_args(&a);
        assert_eq!(p.limit, Some(10));
        assert!(p.json_mode);
        assert!(matches!(p.sort_mode, SortMode::Freq));
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

    // ── Flag position independence ─────────────────────────────────

    #[test]
    fn parse_args_json_list_swapped_still_finds_flags() {
        let a = vec!["loci".to_string(), "-l".to_string(), "--json".to_string()];
        let p = parse_args(&a);
        assert!(p.json_mode);
        assert!(p.list_mode);
    }

    #[test]
    fn parse_args_json_after_filter() {
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
