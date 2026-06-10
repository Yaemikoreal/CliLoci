/// List-mode output: text, JSON, tag filtering, sorting, and metadata.
///
/// Extracted from `main.rs` to keep the output rendering logic
/// independently testable and maintainable.

use std::collections::HashMap;

use crate::args::{self, fuzzy_match, SortMode};
use crate::metadata;
use crate::usage;

/// Run the full list-mode pipeline: filter → metadata → tag-filter → sort → output.
pub fn output_list(
    executables: &[String],
    parsed: &args::ParsedArgs,
    path_dirs: &[std::path::PathBuf],
) {
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
    let meta_cache: Option<HashMap<String, metadata::ToolMeta>> = if meta_mode {
        let owned: Vec<String> = name_filtered.iter().map(|s| (*s).clone()).collect();
        Some(metadata::collect_meta(
            &owned,
            path_dirs,
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
                 meta_mode should have been forced true",
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

    // 5. Apply limit truncation
    if let Some(limit) = parsed.limit {
        sorted.truncate(limit);
    }

    // 6. Count mode short-circuit (skip full output)
    if parsed.count_mode {
        if parsed.json_mode {
            println!(
                "{}",
                serde_json::json!({
                    "skill_version": concat!("v", env!("CARGO_PKG_VERSION")),
                    "total": sorted.len()
                })
            );
        } else {
            println!("{}", sorted.len());
        }
        return;
    }

    // 7. Output
    let sorted_refs: Vec<&String> = sorted.iter().collect();
    if parsed.json_mode {
        output_json(&sorted_refs, &meta_cache, parsed);
    } else {
        for exe in sorted_refs {
            println!("{}", exe);
        }
    }
}

/// Format and print JSON output.
fn output_json(
    sorted_refs: &[&String],
    meta_cache: &Option<HashMap<String, metadata::ToolMeta>>,
    parsed: &args::ParsedArgs,
) {
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
        let sorted_names: Vec<&str> = sorted_refs.iter().map(|s| s.as_str()).collect();
        let sorted_meta: HashMap<String, metadata::ToolMeta> = meta
            .iter()
            .filter(|(name, _)| sorted_names.contains(&name.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        output["meta"] = serde_json::to_value(&sorted_meta).unwrap_or(serde_json::Value::Null);
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
}
