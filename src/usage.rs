//! Usage frequency tracking for loci.
//!
//! Records how often each tool is launched and persists the data
//! to `~/.local/share/loci/usage.json`.  Tools launched more
//! frequently (and more recently) sort higher by default.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageEntry {
    count: u64,
    last: String, // ISO 8601 timestamp
}

/// Path to the usage stats file.
fn usage_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("loci").join("usage.json"))
}

/// Load persisted usage stats.
fn load_raw() -> HashMap<String, UsageEntry> {
    let path = match usage_path() {
        Some(p) => p,
        None => return HashMap::new(),
    };
    if !path.exists() {
        return HashMap::new();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Record a launch of `tool`.  Increments its counter and updates
/// the last-used timestamp.
pub fn record_usage(tool: &str) {
    let mut data = load_raw();
    let now = iso_now();
    let entry = data
        .entry(tool.to_string())
        .or_insert_with(|| UsageEntry {
            count: 0,
            last: now.clone(),
        });
    entry.count += 1;
    entry.last = now;

    // Persist atomically (write tmp, then rename).
    if let Some(path) = usage_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(&data) {
            let tmp = path.with_extension("json.tmp");
            let _ = std::fs::write(&tmp, &json);
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// Return an ISO 8601-like timestamp string for `now`.
fn iso_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    // Format as YYYY-MM-DDTHH:MM:SS (UTC approximation).
    let secs = d.as_secs();
    let (y, mo, dy, h, mi, s) = from_unix_secs(secs);
    format!("{y:04}-{mo:02}-{dy:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Convert Unix seconds to (year, month, day, hour, min, sec) in UTC.
/// Implements the civil calendar with a simplified leap-year algorithm.
fn from_unix_secs(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    const SECS_PER_DAY: u64 = 86400;
    let days = secs / SECS_PER_DAY;
    let rem = secs % SECS_PER_DAY;

    let h = rem / 3600;
    let mi = (rem % 3600) / 60;
    let s = rem % 60;

    // Days since 1970-01-01 (civil).
    let mut y = 1970i64;
    let mut d = days as i64;
    loop {
        let leap = is_leap(y);
        let yd = if leap { 366 } else { 365 };
        if d < yd {
            break;
        }
        d -= yd;
        y += 1;
    }

    let leap = is_leap(y);
    let month_days: &[i64] = if leap {
        &[31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        &[31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut mo = 0;
    for (i, &md) in month_days.iter().enumerate() {
        if d < md {
            mo = i + 1;
            break;
        }
        d -= md;
    }
    if mo == 0 {
        mo = 12;
    }
    // d is now day-of-month (0-based), convert to 1-based.
    (y as u64, mo as u64, (d + 1) as u64, h, mi, s)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Sort executables alphabetically (stable, in-place).
pub fn sort_alpha(executables: &mut [String]) {
    executables.sort();
}

/// Sort executables by usage frequency (most-used first), then by
/// most-recently-used, then alphabetically as a tiebreaker.
pub fn sort_by_frequency(executables: &mut [String]) {
    let usage = load_raw();
    executables.sort_by(|a, b| {
        let ua = usage.get(a);
        let ub = usage.get(b);
        ub.map(|u| u.count)
            .unwrap_or(0)
            .cmp(&ua.map(|u| u.count).unwrap_or(0))
            .then_with(|| {
                ub.map(|u| &u.last[..])
                    .unwrap_or("")
                    .cmp(ua.map(|u| &u.last[..]).unwrap_or(""))
            })
            .then_with(|| a.cmp(b))
    });
}

// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Create the usage.json at the real platform data path and return its parent dir
    /// for cleanup.  After the test, the entire loci data dir is removed.
    fn setup_usage(data: &str) -> std::path::PathBuf {
        let path = usage_path().expect("data dir should be available");
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&path, data).expect("write test usage.json");
        // Return the directory to remove after the test
        path.parent().unwrap().to_path_buf()
    }

    fn teardown_usage(dir: &std::path::Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_from_unix_secs_epoch() {
        let (y, mo, d, h, mi, s) = from_unix_secs(0);
        assert_eq!((y, mo, d, h, mi, s), (1970, 1, 1, 0, 0, 0));
    }

    #[test]
    fn test_from_unix_secs_known() {
        // 2024-01-15T10:30:00Z ≈ 1705314600
        let (y, mo, d, h, mi, s) = from_unix_secs(1705314600);
        assert_eq!((y, mo, d, h, mi, s), (2024, 1, 15, 10, 30, 0));
    }

    #[test]
    fn test_is_leap() {
        assert!(is_leap(2000));
        assert!(!is_leap(1900));
        assert!(is_leap(2024));
        assert!(!is_leap(2023));
    }

    #[test]
    fn test_sort_alpha() {
        let mut v = vec!["c".to_string(), "a".to_string(), "b".to_string()];
        sort_alpha(&mut v);
        assert_eq!(v, vec!["a", "b", "c"]);
    }

    // ── sort_by_frequency ──────────────────────────────────────────

    #[test]
    fn sort_by_frequency_empty_list() {
        let mut v: Vec<String> = vec![];
        sort_by_frequency(&mut v); // should not panic
        assert!(v.is_empty());
    }

    #[test]
    fn sort_by_frequency_no_data_file() {
        // No usage.json exists → sort is effectively a no-op (alpha already sorted)
        let mut v = vec!["b".to_string(), "a".to_string()];
        sort_by_frequency(&mut v);
        // load_raw returns empty HashMap, so all tools have weight 0 → alpha tiebreaker
        assert_eq!(v, vec!["a", "b"]);
    }

    #[test]
    fn sort_by_frequency_with_data() {
        let json = r#"{
            "rarely-used": {"count": 1, "last": "2024-01-01T00:00:00Z"},
            "often-used":  {"count": 99, "last": "2024-06-01T00:00:00Z"},
            "mid-used":    {"count": 10, "last": "2024-03-15T12:00:00Z"}
        }"#;
        let data_dir = setup_usage(json);

        let mut v = vec![
            "mid-used".to_string(),
            "often-used".to_string(),
            "rarely-used".to_string(),
        ];
        sort_by_frequency(&mut v);

        // Expected order: often-used (99) > mid-used (10) > rarely-used (1)
        assert_eq!(v, vec!["often-used", "mid-used", "rarely-used"]);

        teardown_usage(&data_dir);
    }

    #[test]
    fn sort_by_frequency_unknown_tools_at_end() {
        let json = r#"{"known-tool": {"count": 5, "last": "2024-06-01T00:00:00Z"}}"#;
        let data_dir = setup_usage(json);

        let mut v = vec![
            "unknown-tool".to_string(),
            "known-tool".to_string(),
        ];
        sort_by_frequency(&mut v);

        // known-tool has count=5, unknown-tool has count=0 → known first
        assert_eq!(v, vec!["known-tool", "unknown-tool"]);

        teardown_usage(&data_dir);
    }

    #[test]
    fn sort_by_frequency_tiebreaker_recency_then_alpha() {
        let json = r#"{
            "zzz": {"count": 1, "last": "2024-01-01T00:00:00Z"},
            "aaa": {"count": 1, "last": "2024-06-01T00:00:00Z"},
            "mmm": {"count": 1, "last": "2024-03-01T00:00:00Z"}
        }"#;
        let data_dir = setup_usage(json);

        let mut v = vec!["mmm".to_string(), "aaa".to_string(), "zzz".to_string()];
        sort_by_frequency(&mut v);

        // Same count → sorted by recency (most recent first)
        // aaa (June) > mmm (March) > zzz (January)
        assert_eq!(v, vec!["aaa", "mmm", "zzz"]);

        teardown_usage(&data_dir);
    }

    // ── record_usage ───────────────────────────────────────────────

    #[test]
    fn record_usage_creates_file() {
        let path = usage_path().expect("data dir should be available");
        // Ensure clean state
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }

        record_usage("test-tool-a");

        assert!(path.exists(), "usage.json should be created");
        // Clean up
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn record_usage_increments_count() {
        let path = usage_path().expect("data dir should be available");
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }

        record_usage("test-tool-b");
        record_usage("test-tool-b");

        let data = load_raw();
        assert_eq!(data.get("test-tool-b").unwrap().count, 2);

        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn record_usage_two_different_tools() {
        let path = usage_path().expect("data dir should be available");
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }

        record_usage("alpha");
        record_usage("beta");
        record_usage("alpha");

        let data = load_raw();
        assert_eq!(data.get("alpha").unwrap().count, 2);
        assert_eq!(data.get("beta").unwrap().count, 1);

        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    // ── iso_now ────────────────────────────────────────────────────

    #[test]
    fn iso_now_format_matches_expected() {
        let s = iso_now();
        // Expected format: "YYYY-MM-DDTHH:MM:SSZ"
        assert_eq!(s.len(), 20, "ISO 8601 length: got '{}'", s);
        assert!(s.ends_with('Z'), "should end with Z: '{}'", s);
        assert_eq!(&s[4..5], "-", "dash after year: '{}'", s);
        assert_eq!(&s[7..8], "-", "dash after month: '{}'", s);
        assert_eq!(&s[10..11], "T", "T separator: '{}'", s);
        assert_eq!(&s[13..14], ":", "colon after hour: '{}'", s);
        assert_eq!(&s[16..17], ":", "colon after minute: '{}'", s);
        // Verify all components are numeric
        let digits: &[usize] = &[0,1,2,3, 5,6, 8,9, 11,12, 14,15, 17,18];
        for &i in digits {
            assert!(s.as_bytes()[i].is_ascii_digit(), "pos {} not digit: '{}'", i, s);
        }
    }
}
