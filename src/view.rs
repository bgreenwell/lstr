//! Implements the classic, non-interactive directory tree view.

use crate::app::ViewArgs;
use crate::git;
use crate::icons;
use crate::sort;
use crate::utils;
use colored::{control, Colorize};
use ignore::{self, WalkBuilder};
use lscolors::LsColors;
use std::fs;
use std::io::{self, Write};
use url::Url;

/// Executes the classic directory tree view
pub fn run(args: &ViewArgs, ls_colors: &LsColors) -> anyhow::Result<()> {
    if !args.common.path.is_dir() {
        anyhow::bail!("'{}' is not a directory.", args.common.path.display());
    }

    let canonical_root = fs::canonicalize(&args.common.path)?;

    match args.color {
        crate::app::ColorChoice::Always => control::set_override(true),
        crate::app::ColorChoice::Never => control::set_override(false),
        crate::app::ColorChoice::Auto => {}
    }

    // Format root directory with same alignment as tree entries
    let root_metadata = if args.common.size || args.common.permissions {
        fs::metadata(&args.common.path).ok()
    } else {
        None
    };

    let root_permissions_str = if args.common.permissions {
        let perms = root_metadata
            .as_ref()
            .map(utils::permission_string)
            .unwrap_or_else(|| "----------".to_string());
        format!("{perms} ")
    } else {
        String::new()
    };

    let root_git_status_str = if args.common.git_status {
        "  ".to_string() // Empty git status column for consistent spacing
    } else {
        String::new()
    };

    if writeln!(
        io::stdout(),
        "{}{}{}",
        root_git_status_str,
        root_permissions_str,
        args.common.path.display().to_string().blue().bold()
    )
    .is_err()
    {
        return Ok(());
    }

    let git_repo_status =
        if args.common.git_status { git::load_status(&canonical_root)? } else { None };
    let status_cache = git_repo_status.as_ref().map(|s| &s.cache);
    // The walk root's location inside the repo, computed once so each
    // entry's cache key is a cheap path join instead of a canonicalize()
    // syscall per entry.
    let root_in_repo = git_repo_status
        .as_ref()
        .and_then(|s| canonical_root.strip_prefix(&s.root).ok().map(|p| p.to_path_buf()));

    let mut builder = WalkBuilder::new(&args.common.path);
    utils::configure_ignore_filters(&mut builder, args.common.all, args.common.gitignore);
    if let Some(level) = args.level {
        builder.max_depth(Some(level));
    }

    let mut dir_count = 0;
    let mut file_count = 0;

    // Collect all entries first, then sort them
    let mut entries: Vec<_> = builder
        .build()
        .filter_map(|result| match result {
            Ok(entry) => {
                if entry.depth() == 0 {
                    None // Skip the root directory
                } else {
                    Some(entry)
                }
            }
            Err(err) => {
                eprintln!("lstr: ERROR: {err}");
                None
            }
        })
        .collect();

    // Filter before computing tree connectors so that skipped entries do
    // not count as siblings of the entries that are actually printed.
    if args.dirs_only {
        entries.retain(|entry| entry.file_type().is_some_and(|ft| ft.is_dir()));
    }

    // Apply tree-aware sorting (preserves parent-child relationships)
    let sort_options = args.common.to_sort_options();
    sort::sort_entries_hierarchically(&mut entries, &sort_options);

    // The summary counts reflect the whole walked tree, including entries
    // hidden by --file-depth / --max-items (they are represented by the
    // "[+N ...]" markers).
    for entry in &entries {
        if entry.file_type().is_some_and(|ft| ft.is_dir()) {
            dir_count += 1;
        } else {
            file_count += 1;
        }
    }

    let nodes = apply_display_limits(entries, args.file_depth, args.max_items);

    // Build tree structure information
    let depths: Vec<usize> = nodes.iter().map(TreeNode::depth).collect();
    let tree_info = build_tree_info(&depths);

    for (index, node) in nodes.iter().enumerate() {
        let (prefix, connector) = &tree_info[index];

        let entry = match node {
            TreeNode::Entry(entry) => entry,
            TreeNode::Summary { hidden_files, hidden_items, .. } => {
                let git_col =
                    if status_cache.is_some() && root_in_repo.is_some() { "  " } else { "" };
                let perm_col = if args.common.permissions { " ".repeat(11) } else { String::new() };
                let label = summary_label(*hidden_files, *hidden_items);
                if writeln!(
                    io::stdout(),
                    "{}{}{}{} {}",
                    git_col,
                    perm_col,
                    prefix,
                    connector,
                    label.dimmed()
                )
                .is_err()
                {
                    break;
                }
                continue;
            }
        };
        let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());

        let git_status_str = if let (Some(cache), Some(base)) =
            (status_cache, root_in_repo.as_ref())
        {
            entry
                .path()
                .strip_prefix(&args.common.path)
                .ok()
                .and_then(|rel| cache.get(&base.join(rel)))
                .map(|s| {
                    let status_char = s.get_char();
                    let color = match s {
                        git::FileStatus::New | git::FileStatus::Renamed => colored::Color::Green,
                        git::FileStatus::Modified | git::FileStatus::Typechange => {
                            colored::Color::Yellow
                        }
                        git::FileStatus::Deleted => colored::Color::Red,
                        git::FileStatus::Conflicted => colored::Color::BrightRed,
                        git::FileStatus::Untracked => colored::Color::Magenta,
                    };
                    format!("{status_char} ").color(color).to_string()
                })
                .unwrap_or_else(|| "  ".to_string())
        } else {
            String::new()
        };

        let metadata =
            if args.common.size || args.common.permissions { entry.metadata().ok() } else { None };
        let permissions_str = if args.common.permissions {
            let perms = metadata
                .as_ref()
                .map(utils::permission_string)
                .unwrap_or_else(|| "----------".to_string());
            format!("{perms} ")
        } else {
            String::new()
        };

        let name = entry.file_name().to_string_lossy();
        let icon_str = if args.common.icons {
            let (icon, color) = icons::get_icon_for_path(entry.path(), is_dir);
            format!("{} ", icon.color(color))
        } else {
            String::new()
        };
        let size_str = if args.common.size && !is_dir {
            metadata
                .as_ref()
                .map(|m| format!(" ({})", utils::format_size(m.len())))
                .unwrap_or_default()
        } else {
            String::new()
        };

        // --- Corrected Logic Block ---
        let ls_style = ls_colors.style_for_path(entry.path()).cloned().unwrap_or_default();
        let mut styled_name = name.to_string().normal();

        if let Some(fg) = ls_style.foreground {
            use lscolors::Color as LsColor;
            let color = match fg {
                LsColor::Black => colored::Color::Black,
                LsColor::Red => colored::Color::Red,
                LsColor::Green => colored::Color::Green,
                LsColor::Yellow => colored::Color::Yellow,
                LsColor::Blue => colored::Color::Blue,
                LsColor::Magenta => colored::Color::Magenta,
                LsColor::Cyan => colored::Color::Cyan,
                LsColor::White => colored::Color::White,
                LsColor::BrightBlack => colored::Color::BrightBlack,
                LsColor::BrightRed => colored::Color::BrightRed,
                LsColor::BrightGreen => colored::Color::BrightGreen,
                LsColor::BrightYellow => colored::Color::BrightYellow,
                LsColor::BrightBlue => colored::Color::BrightBlue,
                LsColor::BrightMagenta => colored::Color::BrightMagenta,
                LsColor::BrightCyan => colored::Color::BrightCyan,
                LsColor::BrightWhite => colored::Color::BrightWhite,
                LsColor::Fixed(_) => colored::Color::White,
                LsColor::RGB(r, g, b) => colored::Color::TrueColor { r, g, b },
            };
            styled_name = styled_name.color(color);
        }

        if ls_style.font_style.bold {
            styled_name = styled_name.bold();
        }
        if ls_style.font_style.italic {
            styled_name = styled_name.italic();
        }
        if ls_style.font_style.underline {
            styled_name = styled_name.underline();
        }

        // Hyperlink escapes follow the colorization decision so that
        // `--color never` and piped output stay clean, pipeable text.
        let final_name = if args.hyperlinks && !is_dir && control::SHOULD_COLORIZE.should_colorize()
        {
            // Canonicalize the path to get an absolute path for the URL
            if let Ok(abs_path) = fs::canonicalize(entry.path()) {
                if let Ok(url) = Url::from_file_path(abs_path) {
                    format!("\x1B]8;;{url}\x07{styled_name}\x1B]8;;\x07")
                } else {
                    styled_name.to_string()
                }
            } else {
                styled_name.to_string()
            }
        } else {
            styled_name.to_string()
        };

        if writeln!(
            io::stdout(),
            "{}{}{}{} {}{}{}",
            git_status_str,
            permissions_str.dimmed(),
            prefix,
            connector,
            icon_str,
            final_name,
            size_str.dimmed()
        )
        .is_err()
        {
            break;
        }
    }

    let summary = format!("\n{dir_count} directories, {file_count} files");
    _ = writeln!(io::stdout(), "{summary}");

    Ok(())
}

/// A renderable line in the tree: a real entry, or a per-directory summary
/// of entries hidden by `--file-depth` / `--max-items`.
enum TreeNode {
    Entry(ignore::DirEntry),
    Summary { depth: usize, hidden_files: usize, hidden_items: usize },
}

impl TreeNode {
    fn depth(&self) -> usize {
        match self {
            TreeNode::Entry(entry) => entry.depth(),
            TreeNode::Summary { depth, .. } => *depth,
        }
    }
}

/// Applies the display limits, turning entries into renderable nodes. Each
/// directory whose children were suppressed gets one trailing summary node.
/// Entries suppressed by `--max-items` take their whole subtree with them.
fn apply_display_limits(
    entries: Vec<ignore::DirEntry>,
    file_depth: Option<usize>,
    max_items: Option<usize>,
) -> Vec<TreeNode> {
    if file_depth.is_none() && max_items.is_none() {
        return entries.into_iter().map(TreeNode::Entry).collect();
    }

    use std::collections::HashMap;
    let mut children_map: HashMap<std::path::PathBuf, Vec<ignore::DirEntry>> = HashMap::new();
    let mut roots = Vec::new();
    for entry in entries {
        if entry.depth() == 1 {
            roots.push(entry);
        } else if let Some(parent) = entry.path().parent() {
            children_map.entry(parent.to_path_buf()).or_default().push(entry);
        }
    }

    fn emit(
        children: Vec<ignore::DirEntry>,
        children_map: &mut HashMap<std::path::PathBuf, Vec<ignore::DirEntry>>,
        file_depth: Option<usize>,
        max_items: Option<usize>,
        out: &mut Vec<TreeNode>,
    ) {
        let child_depth = children.first().map(|e| e.depth()).unwrap_or(1);
        let mut kept = 0usize;
        let mut hidden_files = 0usize;
        let mut hidden_items = 0usize;
        for child in children {
            let is_dir = child.file_type().is_some_and(|ft| ft.is_dir());
            if !is_dir && file_depth.is_some_and(|limit| child.depth() > limit) {
                hidden_files += 1;
                continue;
            }
            if max_items.is_some_and(|limit| kept >= limit) {
                hidden_items += 1;
                continue;
            }
            kept += 1;
            let grandchildren = children_map.remove(child.path());
            out.push(TreeNode::Entry(child));
            if let Some(grandchildren) = grandchildren {
                emit(grandchildren, children_map, file_depth, max_items, out);
            }
        }
        if hidden_files > 0 || hidden_items > 0 {
            out.push(TreeNode::Summary { depth: child_depth, hidden_files, hidden_items });
        }
    }

    let mut nodes = Vec::new();
    emit(roots, &mut children_map, file_depth, max_items, &mut nodes);
    nodes
}

/// Formats a summary node's label, e.g. "[+3 files]" or "[+1 file, +2 more]".
fn summary_label(hidden_files: usize, hidden_items: usize) -> String {
    let mut parts = Vec::new();
    if hidden_files > 0 {
        let noun = if hidden_files == 1 { "file" } else { "files" };
        parts.push(format!("+{hidden_files} {noun}"));
    }
    if hidden_items > 0 {
        parts.push(format!("+{hidden_items} more"));
    }
    format!("[{}]", parts.join(", "))
}

/// Builds tree structure information for proper connector display.
/// Returns one (prefix, connector) pair per node, in node order.
///
/// Nodes must be in depth-first order (each node's parent precedes it),
/// which `sort_entries_hierarchically` guarantees. Under that invariant a
/// node is the last of its siblings exactly when the next node at a depth
/// less than or equal to its own is strictly shallower, so everything can be
/// computed in two linear passes instead of rescanning the list per node.
fn build_tree_info(depths: &[usize]) -> Vec<(String, &'static str)> {
    // Reverse pass: pending[d] is true when a later node at depth d exists
    // within the same parent scope (deeper flags are cleared whenever a
    // shallower node is seen, since those nodes belong to its subtree).
    let mut is_last = vec![false; depths.len()];
    let mut pending: Vec<bool> = Vec::new();
    for (index, &depth) in depths.iter().enumerate().rev() {
        if pending.len() <= depth {
            pending.resize(depth + 1, false);
        } else {
            pending.truncate(depth + 1);
        }
        is_last[index] = !pending[depth];
        pending[depth] = true;
    }

    // Forward pass: build each node's prefix from its ancestors' last-sibling
    // flags, maintained as a stack indexed by depth.
    let mut tree_info = Vec::with_capacity(depths.len());
    let mut prefix_parts: Vec<&'static str> = Vec::new();
    for (index, &depth) in depths.iter().enumerate() {
        prefix_parts.truncate(depth.saturating_sub(1));
        let prefix = prefix_parts.concat();
        let connector = if is_last[index] { "└──" } else { "├──" };
        // How this node contributes to its descendants' prefixes.
        prefix_parts.push(if is_last[index] { "    " } else { "│   " });
        tree_info.push((prefix, connector));
    }

    tree_info
}
