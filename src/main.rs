mod args;
mod exec;
mod list;
mod metadata;
mod platform;
mod scanner;
mod ui;
mod usage;

use std::process;

/// Report an error: plain text on stderr (always), plus structured JSON
/// on stderr when `json_mode` is true (so AI agents can parse it).
fn report_error(json_mode: bool, code: &str, msg: &str) {
    eprintln!("loci: {}", msg);
    if json_mode {
        let err = serde_json::json!({
            "loci_error": { "code": code, "message": msg }
        });
        // serde_json::to_string is infallible for this shape, so unwrap is safe.
        eprintln!("{}", serde_json::to_string(&err).unwrap());
    }
}

fn main() {
    let cli_args: Vec<String> = std::env::args().collect();

    // ── Argument parsing ────────────────────────────────────────────────
    //
    //   loci -l|--list [...flags...] [filter]
    //   loci [--pick-first | --exact | --index N] [filter...] [-- forwarded-args...]
    let parsed = args::parse_args(&cli_args);

    // ── Collect executables ─────────────────────────────────────────────
    let scanner = scanner::Scanner::new();
    let executables = if parsed.project_mode {
        let proj = scanner.collect_project();
        if proj.is_empty() {
            report_error(
                parsed.json_mode,
                "no_project_tools",
                "no project-local tools found (no package.json / pyvenv.cfg / Cargo.toml detected)",
            );
            process::exit(0);
        }
        proj
    } else {
        scanner.collect()
    };

    if executables.is_empty() {
        report_error(parsed.json_mode, "no_executables", "no executables found in PATH");
        process::exit(1);
    }

    // ── Dispatch ────────────────────────────────────────────────────────
    if parsed.list_mode {
        list::output_list(&executables, &parsed, &scanner.path_dirs());
    } else {
        exec::exec_select(&executables, &parsed);
    }
}
