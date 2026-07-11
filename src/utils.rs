//! Shared utility functions for the lstr application.

/// Configures a directory walker's filtering to match the CLI contract:
/// hidden files are shown only with `-a`, and `.gitignore` plus the other
/// standard ignore files (`.ignore`, global gitignore, `.git/info/exclude`,
/// ignore files in parent directories) apply only with `-g`.
///
/// `WalkBuilder` enables all of these filters by default, so each one must
/// be tied to the flags explicitly.
pub fn configure_ignore_filters(builder: &mut ignore::WalkBuilder, all: bool, gitignore: bool) {
    builder
        .standard_filters(false)
        .hidden(!all)
        .parents(gitignore)
        .ignore(gitignore)
        .git_ignore(gitignore)
        .git_global(gitignore)
        .git_exclude(gitignore);
}

/// Formats a size in bytes into a human-readable string using binary prefixes (KiB, MiB).
pub fn format_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;

    let bytes = bytes as f64;

    if bytes < KIB {
        format!("{bytes} B")
    } else if bytes < MIB {
        format!("{:.1} KiB", bytes / KIB)
    } else if bytes < GIB {
        format!("{:.1} MiB", bytes / MIB)
    } else if bytes < TIB {
        format!("{:.1} GiB", bytes / GIB)
    } else {
        format!("{:.1} TiB", bytes / TIB)
    }
}

/// Formats a file's metadata as an `ls -l`-style permission string,
/// including the file-type character (`d`, `l`, or `-`).
///
/// On non-Unix platforms this returns a `"----------"` placeholder.
pub fn permission_string(metadata: &std::fs::Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let type_char = if metadata.file_type().is_symlink() {
            'l'
        } else if metadata.is_dir() {
            'd'
        } else {
            '-'
        };
        format!("{}{}", type_char, format_permissions(metadata.permissions().mode()))
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        "----------".to_string()
    }
}

/// Formats a Unix file mode into a human-readable string (e.g., "rwxr-xr-x"),
/// including the setuid (`s`/`S`), setgid (`s`/`S`), and sticky (`t`/`T`) bits.
#[cfg(unix)]
pub fn format_permissions(mode: u32) -> String {
    let user_r = if mode & 0o400 != 0 { 'r' } else { '-' };
    let user_w = if mode & 0o200 != 0 { 'w' } else { '-' };
    let user_x = match (mode & 0o100 != 0, mode & 0o4000 != 0) {
        (true, true) => 's',
        (false, true) => 'S',
        (true, false) => 'x',
        (false, false) => '-',
    };
    let group_r = if mode & 0o040 != 0 { 'r' } else { '-' };
    let group_w = if mode & 0o020 != 0 { 'w' } else { '-' };
    let group_x = match (mode & 0o010 != 0, mode & 0o2000 != 0) {
        (true, true) => 's',
        (false, true) => 'S',
        (true, false) => 'x',
        (false, false) => '-',
    };
    let other_r = if mode & 0o004 != 0 { 'r' } else { '-' };
    let other_w = if mode & 0o002 != 0 { 'w' } else { '-' };
    let other_x = match (mode & 0o001 != 0, mode & 0o1000 != 0) {
        (true, true) => 't',
        (false, true) => 'T',
        (true, false) => 'x',
        (false, false) => '-',
    };
    format!("{user_r}{user_w}{user_x}{group_r}{group_w}{group_x}{other_r}{other_w}{other_x}")
}

// Unit tests for utility functions
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.0 KiB");
        assert_eq!(format_size(1536), "1.5 KiB");
        let mib = 1024 * 1024;
        assert_eq!(format_size(mib), "1.0 MiB");
        assert_eq!(format_size(mib + mib / 2), "1.5 MiB");
        let gib = mib * 1024;
        assert_eq!(format_size(gib), "1.0 GiB");
    }

    #[test]
    #[cfg(unix)]
    fn test_format_permissions() {
        // -rwxr-xr-x
        let mode = 0o755;
        assert_eq!(format_permissions(mode), "rwxr-xr-x");
        // -rw-r--r--
        let mode_read = 0o644;
        assert_eq!(format_permissions(mode_read), "rw-r--r--");
        // -rwx------
        let mode_user_only = 0o700;
        assert_eq!(format_permissions(mode_user_only), "rwx------");
    }

    #[test]
    #[cfg(unix)]
    fn test_format_permissions_special_bits() {
        // setuid with execute (e.g. /usr/bin/sudo)
        assert_eq!(format_permissions(0o4755), "rwsr-xr-x");
        // setuid without execute
        assert_eq!(format_permissions(0o4655), "rwSr-xr-x");
        // setgid with execute
        assert_eq!(format_permissions(0o2755), "rwxr-sr-x");
        // sticky bit with execute (e.g. /tmp)
        assert_eq!(format_permissions(0o1777), "rwxrwxrwt");
        // sticky bit without execute
        assert_eq!(format_permissions(0o1776), "rwxrwxrwT");
    }
}
