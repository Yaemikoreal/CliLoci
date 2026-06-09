/// Platform-specific executable detection.
///
/// On Unix: checks the executable permission bit (0o111).
/// On Windows: checks file extension against PATHEXT environment variable.
pub fn is_executable(name: &str, metadata: &std::fs::Metadata) -> bool {
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(windows)]
    {
        let path_ext =
            std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.BAT;.CMD;.COM;.PS1".to_string());
        let ext = std::path::Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let ext_with_dot = format!(".{}", ext.to_uppercase());
        path_ext
            .split(';')
            .any(|pe| pe.eq_ignore_ascii_case(&ext_with_dot))
    }

    #[cfg(not(any(unix, windows)))]
    {
        // Fallback for other platforms: trust any file
        true
    }
}

// ---------------------------------------------------------------------------
// Tests (only compile when testing)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a temporary directory scoped to one test case.
    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "loci-test-platform-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn is_executable_rejects_directory() {
        let dir = temp_dir();
        let meta = fs::metadata(&dir).unwrap();
        assert!(!is_executable("somedir", &meta));
    }

    #[test]
    fn is_executable_rejects_nonexistent() {
        // We can't get metadata for a nonexistent path, so this verifies
        // that is_executable correctly rejects non-file metadata by
        // passing metadata for a directory (which passes is_file() → false).
        let dir = temp_dir();
        let meta = fs::metadata(&dir).unwrap();
        assert!(!is_executable("missing", &meta));
    }

    #[cfg(unix)]
    #[test]
    fn is_executable_unix_permission_bits() {
        use std::os::unix::fs::PermissionsExt;

        let dir = temp_dir();
        let file_path = dir.join("myscript");

        // File without execute bit → not executable
        fs::write(&file_path, "content").unwrap();
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o644)).unwrap();
        let meta = fs::metadata(&file_path).unwrap();
        assert!(!is_executable("myscript", &meta),
            "file 0644 should NOT be executable");

        // File with execute bit → executable
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o755)).unwrap();
        let meta = fs::metadata(&file_path).unwrap();
        assert!(is_executable("myscript", &meta),
            "file 0755 SHOULD be executable");

        // Only owner execute (0o100) → still executable
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o100)).unwrap();
        let meta = fs::metadata(&file_path).unwrap();
        assert!(is_executable("myscript", &meta),
            "file 0100 should be executable (owner execute)");
    }

    #[cfg(windows)]
    #[test]
    fn is_executable_windows_extensions() {
        let dir = temp_dir();

        // .EXE → executable
        let exe = dir.join("tool.EXE");
        fs::write(&exe, "").unwrap();
        let meta = fs::metadata(&exe).unwrap();
        assert!(is_executable("tool.EXE", &meta), ".EXE should be executable");

        // .bat (case-insensitive) → executable
        let bat = dir.join("script.BAT");
        fs::write(&bat, "").unwrap();
        let meta = fs::metadata(&bat).unwrap();
        assert!(is_executable("script.BAT", &meta), ".BAT should be executable");

        // .cmd → executable
        let cmd = dir.join("test.CMD");
        fs::write(&cmd, "").unwrap();
        let meta = fs::metadata(&cmd).unwrap();
        assert!(is_executable("test.CMD", &meta), ".CMD should be executable");

        // .txt → NOT executable
        let txt = dir.join("readme.txt");
        fs::write(&txt, "").unwrap();
        let meta = fs::metadata(&txt).unwrap();
        assert!(!is_executable("readme.txt", &meta), ".txt should NOT be executable");
    }

    #[cfg(not(any(unix, windows)))]
    #[test]
    fn is_executable_other_platform_trusts_files() {
        let dir = temp_dir();
        let f = dir.join("anyfile");
        fs::write(&f, "").unwrap();
        let meta = fs::metadata(&f).unwrap();
        assert!(is_executable("anyfile", &meta),
            "fallback platform should trust any regular file");
    }
}
