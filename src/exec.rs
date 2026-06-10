/// Tool execution and selection dispatch.
///
/// Handles the four selection modes (interactive / pick-first / exact / index)
/// and process launch for each chosen tool.

use std::process::{self, Command};

use crate::args::{self, fuzzy_match};
use crate::usage;

/// Run one of the four selection modes and launch the chosen tool.
/// This function never returns (`-> !`) because it either:
/// - `exec`s the target tool (Unix), replacing the process, or
/// - spawns + waits + exits with the child's exit code (Windows).
pub fn exec_select(executables: &[String], parsed: &args::ParsedArgs) -> ! {
    match parsed.select_mode {
        args::SelectMode::Interactive => {
            let selected = crate::ui::select_executable(executables, parsed.filter.as_deref());
            if let Some(tool) = selected {
                usage::record_usage(&tool);
                launch(&tool, &parsed.forwarded, parsed.json_mode);
            }
            process::exit(0); // user cancelled
        }
        args::SelectMode::PickFirst => {
            let filtered: Vec<&String> = match &parsed.filter {
                Some(f) => executables.iter().filter(|e| fuzzy_match(e, f)).collect(),
                None => executables.iter().collect(),
            };
            if let Some(tool) = filtered.first() {
                usage::record_usage(tool);
                launch(tool, &parsed.forwarded, parsed.json_mode);
            }
            crate::report_error(
                parsed.json_mode,
                "no_matching_tool",
                &format!(
                    "no matching tool for '{}'",
                    parsed.filter.as_deref().unwrap_or("")
                ),
            );
            process::exit(1);
        }
        args::SelectMode::Exact => {
            if let Some(f) = &parsed.filter {
                if let Some(tool) = executables.iter().find(|e| *e == f) {
                    usage::record_usage(tool);
                    launch(tool, &parsed.forwarded, parsed.json_mode);
                }
            }
            crate::report_error(
                parsed.json_mode,
                "exact_not_found",
                &format!(
                    "exact match not found for '{}'",
                    parsed.filter.as_deref().unwrap_or("")
                ),
            );
            process::exit(1);
        }
        args::SelectMode::Index(idx) => {
            let filtered: Vec<&String> = match &parsed.filter {
                Some(f) => executables.iter().filter(|e| fuzzy_match(e, f)).collect(),
                None => executables.iter().collect(),
            };
            if idx < filtered.len() {
                usage::record_usage(filtered[idx]);
                launch(filtered[idx], &parsed.forwarded, parsed.json_mode);
            }
            crate::report_error(
                parsed.json_mode,
                "index_out_of_range",
                &format!(
                    "index {} out of range (0..{}) for '{}'",
                    idx,
                    filtered.len(),
                    parsed.filter.as_deref().unwrap_or("")
                ),
            );
            process::exit(1);
        }
    }
}

/// Replace the current process with the selected tool (Unix `exec`)
/// or spawn-and-wait (Windows fallback).  stdin/stdout/stderr are
/// inherited automatically.
fn launch(tool: &str, args: &[&str], json_mode: bool) -> ! {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = Command::new(tool).args(args).exec();
        // exec() only returns on error
        crate::report_error(
            json_mode,
            "exec_failed",
            &format!("failed to execute '{}': {}", tool, err),
        );
        process::exit(1);
    }

    #[cfg(not(unix))]
    {
        let status = Command::new(tool).args(args).status().unwrap_or_else(|e| {
            crate::report_error(
                json_mode,
                "exec_failed",
                &format!("failed to execute '{}': {}", tool, e),
            );
            process::exit(1);
        });
        match status.code() {
            Some(code) => process::exit(code),
            None => {
                crate::report_error(
                    json_mode,
                    "exec_failed",
                    "child process was terminated abnormally (exit code unavailable)",
                );
                process::exit(1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    // Verify the module compiles and basic dispatch works.
    // Full integration tests are in tests/cli.rs.
}
