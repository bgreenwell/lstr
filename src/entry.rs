//! A tree entry that may come from a live filesystem walk (`ignore::DirEntry`)
//! or from a user-supplied listing (`--fromfile`).
//!
//! `sort::sort_entries_hierarchically`'s DFS reconstruction only depends on
//! `path()`/`depth()`, not on where the entry came from, which is what lets
//! both kinds of entry share the same sorting and rendering pipeline in
//! `view.rs`.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::Metadata;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// A single node in the displayed tree.
pub enum TreeEntry {
    /// Produced by walking the real filesystem.
    Walked(ignore::DirEntry),
    /// Synthesized from a `--fromfile` path list.
    Listed(ListedEntry),
}

/// A synthetic entry parsed from `--fromfile` input. `is_dir` is inferred
/// from the input (a trailing `/`, or being an ancestor of another entry)
/// and is trusted as-is; it is never confirmed against the real filesystem.
pub struct ListedEntry {
    pub path: PathBuf,
    pub depth: usize,
    pub is_dir: bool,
}

impl TreeEntry {
    pub fn path(&self) -> &Path {
        match self {
            TreeEntry::Walked(e) => e.path(),
            TreeEntry::Listed(e) => &e.path,
        }
    }

    pub fn depth(&self) -> usize {
        match self {
            TreeEntry::Walked(e) => e.depth(),
            TreeEntry::Listed(e) => e.depth,
        }
    }

    pub fn file_name(&self) -> &OsStr {
        match self {
            TreeEntry::Walked(e) => e.file_name(),
            TreeEntry::Listed(e) => e.path.file_name().unwrap_or(e.path.as_os_str()),
        }
    }

    /// Whether this entry is a directory. For `Listed` entries this is the
    /// flag inferred by `parse_fromfile` from the input list — it is
    /// trusted as-is and never confirmed with a `stat()`, unlike
    /// `metadata()` below. This asymmetry is intentional: `--fromfile`
    /// trusts the listing for tree *structure*, and only opportunistically
    /// consults the real filesystem for size/permissions when the caller
    /// actually asks for them.
    pub fn is_dir(&self) -> bool {
        match self {
            TreeEntry::Walked(e) => e.file_type().is_some_and(|ft| ft.is_dir()),
            TreeEntry::Listed(e) => e.is_dir,
        }
    }

    /// Real filesystem metadata for the entry, when available. `Listed`
    /// entries are stat'd on demand against the real path — callers already
    /// treat a missing result as "unknown" (blank size / `----------`
    /// permissions). Uses `symlink_metadata` (never follows symlinks) to
    /// match `ignore::DirEntry::metadata()`'s behavior in this codebase,
    /// since `WalkBuilder` is never configured with `follow_links(true)`.
    pub fn metadata(&self) -> Option<Metadata> {
        match self {
            TreeEntry::Walked(e) => e.metadata().ok(),
            TreeEntry::Listed(e) => std::fs::symlink_metadata(&e.path).ok(),
        }
    }
}

impl crate::sort::SortableEntry for TreeEntry {
    fn path(&self) -> &Path {
        TreeEntry::path(self)
    }

    fn depth(&self) -> usize {
        TreeEntry::depth(self)
    }

    fn file_name(&self) -> &OsStr {
        TreeEntry::file_name(self)
    }

    fn is_dir(&self) -> bool {
        TreeEntry::is_dir(self)
    }

    fn size(&self) -> u64 {
        if self.is_dir() {
            0
        } else {
            self.metadata().map(|m| m.len()).unwrap_or(0)
        }
    }

    fn modified(&self) -> Option<std::time::SystemTime> {
        self.metadata().and_then(|m| m.modified().ok())
    }
}

/// Parses `--fromfile` input into an unordered list of synthetic entries
/// rooted at `root`. Ancestor directories are inferred from path
/// components; a line ending in `/` marks an otherwise-childless path as an
/// explicit empty directory. Hidden components are dropped unless `all` is
/// set, mirroring the real walk's `-a` behavior. Blank lines are ignored,
/// and duplicate paths collapse into one entry.
///
/// The returned order is unspecified — `sort::sort_entries_hierarchically`
/// reconstructs depth-first order from each entry's path and depth
/// regardless of input order.
pub fn parse_fromfile(
    reader: impl BufRead,
    root: &Path,
    all: bool,
    max_depth: Option<usize>,
) -> anyhow::Result<Vec<TreeEntry>> {
    let mut dirs: HashMap<PathBuf, bool> = HashMap::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.strip_suffix('\r').unwrap_or(&line);
        if line.is_empty() {
            continue;
        }

        let line_is_dir = line.ends_with('/');
        let line = line.strip_prefix("./").unwrap_or(line);
        let line = line.trim_end_matches('/');

        let components: Vec<&str> = line.split('/').filter(|c| !c.is_empty()).collect();
        if components.is_empty() {
            continue;
        }
        if !all && components.iter().any(|c| c.starts_with('.')) {
            continue;
        }

        // A `max_depth` limit truncates the line rather than dropping it
        // outright, matching `WalkBuilder::max_depth`'s `depth <= level`
        // semantics: a directory at the depth limit is still shown, just
        // without its deeper contents. The component at the truncation
        // point is therefore always a directory (it has children beyond
        // the limit), regardless of the line's own trailing slash.
        let limit_len = max_depth.map_or(components.len(), |limit| components.len().min(limit));
        if limit_len == 0 {
            continue;
        }
        let truncated = limit_len < components.len();

        let mut prefix = PathBuf::new();
        let last = limit_len - 1;
        for (index, component) in components.iter().take(limit_len).enumerate() {
            prefix.push(component);
            if index < last {
                dirs.entry(prefix.clone()).and_modify(|d| *d = true).or_insert(true);
            } else {
                let is_dir_flag = truncated || line_is_dir;
                dirs.entry(prefix.clone())
                    .and_modify(|d| *d = *d || is_dir_flag)
                    .or_insert(is_dir_flag);
            }
        }
    }

    let entries = dirs
        .into_iter()
        .map(|(rel_path, is_dir)| {
            let depth = rel_path.components().count();
            TreeEntry::Listed(ListedEntry { path: root.join(&rel_path), depth, is_dir })
        })
        .collect();
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(lines: &[&str], root: &Path, all: bool, max_depth: Option<usize>) -> Vec<TreeEntry> {
        let input = lines.join("\n");
        parse_fromfile(input.as_bytes(), root, all, max_depth).unwrap()
    }

    fn find<'a>(entries: &'a [TreeEntry], rel: &str) -> Option<&'a TreeEntry> {
        entries.iter().find(|e| e.path().ends_with(rel))
    }

    #[test]
    fn dir_inferred_from_trailing_slash() {
        let entries = parse(&["a/"], Path::new("/root"), false, None);
        assert_eq!(entries.len(), 1);
        assert!(find(&entries, "a").unwrap().is_dir());
    }

    #[test]
    fn dir_inferred_from_ancestor_prefix() {
        let entries = parse(&["a/b.txt"], Path::new("/root"), false, None);
        assert_eq!(entries.len(), 2);
        assert!(find(&entries, "a").unwrap().is_dir());
        assert!(!find(&entries, "a/b.txt").unwrap().is_dir());
    }

    #[test]
    fn leaf_dir_flag_does_not_downgrade_ancestor_either_order() {
        let forward = parse(&["a/b.txt", "a/"], Path::new("/root"), false, None);
        assert!(find(&forward, "a").unwrap().is_dir());

        let reverse = parse(&["a/", "a/b.txt"], Path::new("/root"), false, None);
        assert!(find(&reverse, "a").unwrap().is_dir());
    }

    #[test]
    fn ancestor_wins_over_contradictory_bare_leaf() {
        let entries = parse(&["a.txt", "a.txt/nested.txt"], Path::new("/root"), false, None);
        assert!(find(&entries, "a.txt").unwrap().is_dir());
        assert!(!find(&entries, "a.txt/nested.txt").unwrap().is_dir());
    }

    #[test]
    fn hidden_components_dropped_entirely_when_not_all() {
        let entries = parse(&[".git/config"], Path::new("/root"), false, None);
        assert!(entries.is_empty());

        let entries = parse(&["visible/.hidden/file.txt"], Path::new("/root"), false, None);
        assert!(entries.is_empty());
    }

    #[test]
    fn hidden_components_kept_when_all() {
        let entries = parse(&[".git/config"], Path::new("/root"), true, None);
        assert_eq!(entries.len(), 2);
        assert!(find(&entries, ".git").unwrap().is_dir());
        assert!(!find(&entries, ".git/config").unwrap().is_dir());
    }

    #[test]
    fn max_depth_truncates_like_a_real_walk() {
        // A line deeper than the limit is truncated to the ancestor at the
        // limit, which becomes a directory (it has children beyond the
        // limit) — matching `WalkBuilder::max_depth`, where entries up to
        // and including the limit are still yielded, just without their
        // deeper contents.
        let entries = parse(&["a/b/c.txt"], Path::new("/root"), false, Some(2));
        assert_eq!(entries.len(), 2);
        assert!(find(&entries, "a").unwrap().is_dir());
        assert!(find(&entries, "a/b").unwrap().is_dir());

        let entries = parse(&["a/b/c.txt"], Path::new("/root"), false, Some(3));
        assert_eq!(entries.len(), 3);
        assert!(!find(&entries, "a/b/c.txt").unwrap().is_dir());
    }

    #[test]
    fn blank_lines_crlf_dot_slash_and_double_slash_are_handled() {
        let entries = parse(&["", "./a//b.txt\r", ""], Path::new("/root"), false, None);
        assert_eq!(entries.len(), 2);
        assert!(find(&entries, "a").unwrap().is_dir());
        assert!(!find(&entries, "a/b.txt").unwrap().is_dir());
    }

    #[test]
    fn duplicate_lines_deduplicate() {
        let entries = parse(&["a/b.txt", "a/b.txt"], Path::new("/root"), false, None);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn entries_are_rooted_at_the_given_path() {
        let entries = parse(&["a/b.txt"], Path::new("/root"), false, None);
        let leaf = find(&entries, "a/b.txt").unwrap();
        assert_eq!(leaf.path(), Path::new("/root/a/b.txt"));
    }
}
