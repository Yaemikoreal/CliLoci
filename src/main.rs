mod platform;
mod scanner;
mod ui;

use std::process::{self, Command};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // ── Argument parsing ────────────────────────────────────────────
    //
    //   loci -l|--list [--json] [filter]    print executables and exit
    //   loci [filter...] [-- forwarded-args...]
    //
    // Everything before `--` is a pre-filter string passed to skim.
    // Everything after `--` is forwarded to the selected executable.
    let (list_mode, json_mode, filter, forwarded) = parse_args(&args);

    // ── Collect executables ─────────────────────────────────────────
    let scanner = scanner::Scanner::new();
    let executables = scanner.collect();

    if executables.is_empty() {
        eprintln!("loci: no executables found in PATH");
        process::exit(1);
    }

    // ── List mode: print and exit ───────────────────────────────────
    if list_mode {
        let filtered: Vec<&String> = match &filter {
            Some(f) => executables
                .iter()
                .filter(|e| fuzzy_match(e, f))
                .collect(),
            None => executables.iter().collect(),
        };

        if json_mode {
            let names: Vec<&str> = filtered.iter().map(|s| s.as_str()).collect();
            let output = serde_json::json!({
                "total": names.len(),
                "executables": names,
                "filter": filter,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            for exe in filtered {
                println!("{}", exe);
            }
        }
        process::exit(0);
    }

    // ── Interactive selection ───────────────────────────────────────
    let selected = ui::select_executable(&executables, filter.as_deref());

    let tool = match selected {
        Some(t) => t,
        None => process::exit(0), // user cancelled
    };

    // ── Launch selected tool ────────────────────────────────────────
    launch(&tool, &forwarded);
}

/// Parse CLI arguments into (list_mode, json_mode, filter, forwarded_args).
fn parse_args(args: &[String]) -> (bool, bool, Option<String>, Vec<&str>) {
    if args.len() <= 1 {
        return (false, false, None, vec![]);
    }

    // Scan for --json anywhere in args (position-independent)
    let json_mode = args[1..].iter().any(|a| a == "--json");

    // Check for -l / --list flag
    let list_mode = args[1] == "-l" || args[1] == "--list";
    let start = if list_mode { 2 } else { 1 };

    if start >= args.len() {
        return (list_mode, json_mode, None, vec![]);
    }

    // Collect filter tokens, skipping --json (which would otherwise
    // be consumed as a filter string in e.g. `loci -l --json`).
    let rest: Vec<&str> = args[start..]
        .iter()
        .filter(|a| a.as_str() != "--json")
        .map(String::as_str)
        .collect();

    let dashdash = rest.iter().position(|a| *a == "--");

    let filter = match dashdash {
        Some(0) => None,
        Some(pos) => Some(rest[..pos].join(" ")),
        None if !rest.is_empty() => Some(rest.join(" ")),
        None => None,
    };

    let forwarded: Vec<&str> = dashdash
        .map(|pos| rest[pos + 1..].to_vec())
        .unwrap_or_default();

    (list_mode, json_mode, filter, forwarded)
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
        process::exit(status.code().unwrap_or(1));
    }
}
