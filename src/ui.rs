use skim::prelude::*;

/// Launch the skim fuzzy-finder with the given executable names.
/// Returns `Some(name)` if the user selected an item, or `None` if
/// they aborted (Esc / Ctrl-C).
pub fn select_executable(executables: &[String], filter: Option<&str>) -> Option<String> {
    let total = executables.len();

    let mut builder = SkimOptionsBuilder::default();
    if let Some(f) = filter {
        builder.query(f.to_string());
    }
    let options = builder
        .prompt("loci > ".to_string())
        .header(format!("{} executables  │  type to filter, Enter to select, Esc to quit", total))
        .build()
        .ok()?;

    // Join all executable names into newline-separated input for skim
    let input = executables.join("\n");
    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(std::io::Cursor::new(input));

    let output = Skim::run_with(options, Some(items)).ok()?;

    if output.is_abort {
        return None;
    }

    output
        .selected_items
        .first()
        .map(|item| item.output().to_string())
}
