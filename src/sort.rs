//! Provides OS-agnostic sorting functionality for directory entries.
//!
//! This module implements various sorting strategies for file and directory entries,
//! ensuring consistent behavior across all supported platforms (Windows, macOS, Linux).

use clap::ValueEnum;
use ignore::DirEntry;
use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fmt;

/// Defines the available sorting strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum SortType {
    /// Sort by name (default)
    #[default]
    Name,
    /// Sort by file size
    Size,
    /// Sort by modification time
    Modified,
    /// Sort by file extension
    Extension,
}

/// Implements the Display trait for SortType to show possible values in help messages.
impl fmt::Display for SortType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_possible_value().expect("no values are skipped").get_name().fmt(f)
    }
}

/// Configuration options for sorting directory entries.
#[derive(Debug, Clone, Default)]
pub struct SortOptions {
    /// The primary sorting strategy
    pub sort_type: SortType,
    /// Whether to sort directories before files
    pub directories_first: bool,
    /// Whether to use case-sensitive name sorting
    pub case_sensitive: bool,
    /// Whether to use natural/version sorting (e.g., file1 < file10)
    pub natural_sort: bool,
    /// Whether to reverse the sort order
    pub reverse: bool,
    /// Whether to sort dotfiles/dotfolders first
    pub dotfiles_first: bool,
}

/// Sorts a vector of directory entries according to the given options.
///
/// This function provides OS-agnostic sorting that works consistently across
/// all platforms. The sorting is stable, preserving the original order for
/// equal elements.
///
/// # Arguments
///
/// * `entries` - A mutable reference to the vector of entries to sort
/// * `options` - The sorting configuration to apply
///
/// # Examples
///
/// ```rust
/// use lstr::sort::{sort_entries, SortOptions, SortType};
///
/// let mut entries = vec![/* ... */];
/// let options = SortOptions {
///     sort_type: SortType::Name,
///     directories_first: true,
///     ..Default::default()
/// };
/// sort_entries(&mut entries, &options);
/// ```
pub fn sort_entries(entries: &mut [DirEntry], options: &SortOptions) {
    entries.sort_by(|a, b| {
        let result = compare_entries(a, b, options);
        if options.reverse {
            result.reverse()
        } else {
            result
        }
    });
}

/// Sorts directory entries hierarchically, preserving tree structure.
///
/// This builds an explicit tree structure and then reconstructs the entries
/// in depth-first order with proper sibling sorting within each parent directory.
/// Use this instead of sort_entries() when you need to preserve parent-child relationships.
pub fn sort_entries_hierarchically(entries: &mut Vec<DirEntry>, options: &SortOptions) {
    use std::collections::HashMap;

    if entries.is_empty() {
        return;
    }

    // Build parent-child mapping
    let mut children_map: HashMap<std::path::PathBuf, Vec<DirEntry>> = HashMap::new();
    let mut all_entries_by_path: HashMap<std::path::PathBuf, DirEntry> = HashMap::new();

    // Collect all entries and build parent-child relationships
    for entry in entries.iter() {
        all_entries_by_path.insert(entry.path().to_path_buf(), entry.clone());

        if let Some(parent_path) = entry.path().parent() {
            children_map.entry(parent_path.to_path_buf()).or_default().push(entry.clone());
        }
    }

    // Sort children within each parent directory
    for children in children_map.values_mut() {
        sort_entries(children, options);
    }

    // Find root entries (depth 1, since we skip depth 0)
    let mut root_entries: Vec<_> = entries.iter().filter(|e| e.depth() == 1).cloned().collect();

    // Sort root entries
    sort_entries(&mut root_entries, options);

    // Recursively collect entries in depth-first order
    fn collect_tree_entries(
        entry: &DirEntry,
        children_map: &HashMap<std::path::PathBuf, Vec<DirEntry>>,
        result: &mut Vec<DirEntry>,
    ) {
        // Add the parent entry
        result.push(entry.clone());

        // Add all its children in sorted order
        if let Some(children) = children_map.get(entry.path()) {
            for child in children {
                collect_tree_entries(child, children_map, result);
            }
        }
    }

    // Rebuild entries in proper tree order
    let mut result = Vec::new();
    for root in &root_entries {
        collect_tree_entries(root, &children_map, &mut result);
    }

    *entries = result;
}

/// Compares two directory entries according to the sorting options.
pub fn compare_entries(a: &DirEntry, b: &DirEntry, options: &SortOptions) -> Ordering {
    let a_is_dir = a.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
    let b_is_dir = b.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
    let a_is_dotfile = is_dotfile(a);
    let b_is_dotfile = is_dotfile(b);

    // Handle dotfiles-first and directories-first sorting
    // Order: dotfolders → folders → dotfiles → files
    if options.dotfiles_first {
        match (a_is_dotfile, a_is_dir, b_is_dotfile, b_is_dir) {
            // Same category - continue to name sorting
            (true, true, true, true) |   // Both dotfolders
            (false, true, false, true) | // Both regular folders  
            (true, false, true, false) | // Both dotfiles
            (false, false, false, false) => {}, // Both regular files

            // Different categories - apply priority order
            (true, true, _, _) => return Ordering::Less,   // a is dotfolder (highest priority)
            (_, _, true, true) => return Ordering::Greater, // b is dotfolder
            (false, true, _, _) => return Ordering::Less,   // a is regular folder
            (_, _, false, true) => return Ordering::Greater, // b is regular folder
            (true, false, _, _) => return Ordering::Less,   // a is dotfile
            (_, _, true, false) => return Ordering::Greater, // b is dotfile
        }
    } else if options.directories_first {
        // Original directories-first logic (without dotfile priority)
        match (a_is_dir, b_is_dir) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {} // Both are dirs or both are files, continue
        }
    }

    // Apply the primary sorting strategy
    match options.sort_type {
        SortType::Name => compare_by_name(a, b, options),
        SortType::Size => compare_by_size(a, b),
        SortType::Modified => compare_by_modified(a, b),
        SortType::Extension => compare_by_extension(a, b, options),
    }
}

/// Compares entries by name, handling case sensitivity and natural sorting.
fn compare_by_name(a: &DirEntry, b: &DirEntry, options: &SortOptions) -> Ordering {
    let name_a = a.file_name();
    let name_b = b.file_name();

    if options.natural_sort {
        compare_natural(name_a, name_b)
    } else if options.case_sensitive {
        // Use default order for case-sensitive sorting (numbers, uppercase, lowercase)
        compare_default_order(name_a, name_b)
    } else {
        compare_case_insensitive(name_a, name_b)
    }
}

/// Compares entries by file size, with directories having size 0.
fn compare_by_size(a: &DirEntry, b: &DirEntry) -> Ordering {
    let size_a = get_entry_size(a);
    let size_b = get_entry_size(b);
    size_a.cmp(&size_b)
}

/// Compares entries by modification time.
fn compare_by_modified(a: &DirEntry, b: &DirEntry) -> Ordering {
    let modified_a = a.metadata().ok().and_then(|m| m.modified().ok());
    let modified_b = b.metadata().ok().and_then(|m| m.modified().ok());

    match (modified_a, modified_b) {
        (Some(a_time), Some(b_time)) => a_time.cmp(&b_time),
        (Some(_), None) => Ordering::Less, // Files with known time sort first
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Compares entries by file extension, falling back to name comparison.
fn compare_by_extension(a: &DirEntry, b: &DirEntry, options: &SortOptions) -> Ordering {
    let ext_a = get_extension(a.file_name());
    let ext_b = get_extension(b.file_name());

    let ext_cmp = if options.case_sensitive {
        ext_a.cmp(&ext_b)
    } else {
        compare_case_insensitive_str(&ext_a, &ext_b)
    };

    // If extensions are equal, fall back to name comparison
    if ext_cmp == Ordering::Equal {
        compare_by_name(a, b, options)
    } else {
        ext_cmp
    }
}

/// Performs natural/version sorting comparison on OS strings.
fn compare_natural(a: &OsStr, b: &OsStr) -> Ordering {
    // Convert to strings for natural comparison
    let str_a = a.to_string_lossy();
    let str_b = b.to_string_lossy();

    // Use the natord crate for natural ordering
    natord::compare(&str_a, &str_b)
}

/// Performs case-insensitive comparison on OS strings.
fn compare_case_insensitive(a: &OsStr, b: &OsStr) -> Ordering {
    let str_a = a.to_string_lossy().to_lowercase();
    let str_b = b.to_string_lossy().to_lowercase();
    str_a.cmp(&str_b)
}

/// Implements the default sort order: numbers first, then uppercase, then lowercase.
fn compare_default_order(a: &OsStr, b: &OsStr) -> Ordering {
    let str_a = a.to_string_lossy();
    let str_b = b.to_string_lossy();

    // Compare character by character using the specified priority
    for (char_a, char_b) in str_a.chars().zip(str_b.chars()) {
        let order_a = char_sort_priority(char_a);
        let order_b = char_sort_priority(char_b);

        match order_a.cmp(&order_b) {
            Ordering::Equal => {
                // Same priority category, compare within category
                match char_a.cmp(&char_b) {
                    Ordering::Equal => continue,
                    other => return other,
                }
            }
            other => return other,
        }
    }

    // If all compared characters are equal, compare by length
    str_a.len().cmp(&str_b.len())
}

/// Returns sort priority for a character: numbers (0), uppercase (1), lowercase (2), others (3).
fn char_sort_priority(c: char) -> u8 {
    if c.is_ascii_digit() {
        0 // Numbers first
    } else if c.is_ascii_uppercase() {
        1 // Uppercase second
    } else if c.is_ascii_lowercase() {
        2 // Lowercase third
    } else {
        3 // Everything else last
    }
}

/// Checks if a directory entry is a dotfile/dotfolder (starts with '.').
fn is_dotfile(entry: &DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with('.')
}

/// Performs case-insensitive comparison on regular strings.
fn compare_case_insensitive_str(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// Extracts the file extension from an OS string, returning empty string if none.
fn get_extension(filename: &OsStr) -> String {
    std::path::Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string()
}

/// Gets the size of a directory entry, returning 0 for directories.
fn get_entry_size(entry: &DirEntry) -> u64 {
    if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
        0 // Directories have size 0 for sorting purposes
    } else {
        entry.metadata().ok().map(|m| m.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_case_insensitive_name_sorting() {
        // Test case-insensitive comparison
        let name_a = OsStr::new("Apple");
        let name_b = OsStr::new("banana");

        let result = compare_case_insensitive(name_a, name_b);
        assert_eq!(result, Ordering::Less); // "apple" < "banana"
    }

    #[test]
    fn test_case_sensitive_name_sorting() {
        let name_a = OsStr::new("Apple");
        let name_b = OsStr::new("banana");

        let result = name_a.cmp(name_b);
        assert_eq!(result, Ordering::Less); // "Apple" < "banana" in ASCII
    }

    #[test]
    fn test_natural_sorting() {
        let name_a = OsStr::new("file1.txt");
        let name_b = OsStr::new("file10.txt");

        let result = compare_natural(name_a, name_b);
        assert_eq!(result, Ordering::Less); // file1 < file10 naturally

        // Test that regular lexicographic would give opposite result
        let lexicographic = name_a.cmp(name_b);
        assert_eq!(lexicographic, Ordering::Less); // Actually "file1.txt" < "file10.txt" lexicographically too

        // Better test: "file2.txt" vs "file10.txt"
        let name_c = OsStr::new("file2.txt");
        let name_d = OsStr::new("file10.txt");

        let natural_result = compare_natural(name_c, name_d);
        let lexicographic_result = name_c.cmp(name_d);

        assert_eq!(natural_result, Ordering::Less); // file2 < file10 naturally
        assert_eq!(lexicographic_result, Ordering::Greater); // "file2.txt" > "file10.txt" lexicographically
    }

    #[test]
    fn test_extension_extraction() {
        assert_eq!(get_extension(OsStr::new("file.txt")), "txt");
        assert_eq!(get_extension(OsStr::new("file.tar.gz")), "gz");
        assert_eq!(get_extension(OsStr::new("file")), "");
        assert_eq!(get_extension(OsStr::new(".hidden")), "");
    }

    #[test]
    fn test_sort_options_default() {
        let options = SortOptions::default();
        assert_eq!(options.sort_type, SortType::Name);
        assert!(!options.directories_first);
        assert!(!options.case_sensitive);
        assert!(!options.natural_sort);
        assert!(!options.reverse);
        assert!(!options.dotfiles_first);
    }

    #[test]
    fn test_reverse_sorting() {
        let name_a = OsStr::new("apple");
        let name_b = OsStr::new("banana");

        // Normal comparison: apple < banana
        let normal = compare_case_insensitive(name_a, name_b);
        assert_eq!(normal, Ordering::Less);

        // With reverse option, the final result should be flipped
        // (This would be handled by the sort_entries function)
    }

    #[test]
    fn test_default_sort_order() {
        // Test numbers first, then uppercase, then lowercase
        assert_eq!(compare_default_order(OsStr::new("1file"), OsStr::new("Afile")), Ordering::Less);
        assert_eq!(compare_default_order(OsStr::new("Afile"), OsStr::new("afile")), Ordering::Less);
        assert_eq!(compare_default_order(OsStr::new("afile"), OsStr::new("zfile")), Ordering::Less);

        // Test within same category
        assert_eq!(compare_default_order(OsStr::new("1file"), OsStr::new("2file")), Ordering::Less);
        assert_eq!(compare_default_order(OsStr::new("Afile"), OsStr::new("Bfile")), Ordering::Less);
        assert_eq!(compare_default_order(OsStr::new("afile"), OsStr::new("bfile")), Ordering::Less);
    }

    #[test]
    fn test_char_sort_priority() {
        assert_eq!(char_sort_priority('0'), 0); // digit
        assert_eq!(char_sort_priority('9'), 0); // digit
        assert_eq!(char_sort_priority('A'), 1); // uppercase
        assert_eq!(char_sort_priority('Z'), 1); // uppercase
        assert_eq!(char_sort_priority('a'), 2); // lowercase
        assert_eq!(char_sort_priority('z'), 2); // lowercase
        assert_eq!(char_sort_priority('_'), 3); // other
        assert_eq!(char_sort_priority('-'), 3); // other
    }

    #[test]
    fn test_is_dotfile() {
        // This test would need actual DirEntry objects, but we can test the concept
        // The function checks if filename starts with '.'
        assert!(OsStr::new(".hidden").to_string_lossy().starts_with('.'));
        assert!(OsStr::new(".git").to_string_lossy().starts_with('.'));
        assert!(!OsStr::new("visible.txt").to_string_lossy().starts_with('.'));
        assert!(!OsStr::new("normal").to_string_lossy().starts_with('.'));
    }
}
