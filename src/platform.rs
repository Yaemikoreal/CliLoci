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
