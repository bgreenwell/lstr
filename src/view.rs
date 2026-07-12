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

    let text_mode = args.output == crate::app::OutputFormat::Text;

    if text_mode {
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

    // Cumulative sizes are computed over the full entry list, before any
    // display limits, so a directory's size stays truthful even when its
    // children are hidden.
    let du_sizes = if args.du { Some(compute_cumulative_sizes(&entries)) } else { None };
    let du_map = du_sizes.as_ref().map(|(map, _)| map);
    let du_total = du_sizes.as_ref().map(|(_, total)| *total);
    let size_enabled = args.common.size || args.du;

    let nodes = apply_display_limits(entries, args.file_depth, args.max_items);

    if !text_mode {
        let json = render_json(
            args,
            &nodes,
            dir_count,
            file_count,
            status_cache,
            root_in_repo.as_ref(),
            du_map,
            du_total,
        );
        let _ = writeln!(io::stdout(), "{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

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
            if size_enabled || args.common.permissions { entry.metadata().ok() } else { None };
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
        let size_str = if size_enabled && is_dir {
            // Directories only carry a size under --du (cumulative).
            du_map
                .and_then(|map| map.get(entry.path()))
                .map(|size| format!(" ({})", utils::format_size(*size)))
                .unwrap_or_default()
        } else if size_enabled {
            metadata
                .as_ref()
                .map(|m| format!(" ({})", utils::format_size(m.len())))
                .unwrap_or_default()
        } else {
            String::new()
        };

        let ls_style = ls_colors.style_for_path(entry.path()).cloned().unwrap_or_default();
        let mut styled_name = name.to_string().normal();

        if let Some(fg) = ls_style.foreground {
            styled_name = styled_name.color(crate::color::ls_to_colored(fg));
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

    let summary = match du_total {
        Some(total) => format!(
            "\n{} used in {dir_count} directories, {file_count} files",
            utils::format_size(total)
        ),
        None => format!("\n{dir_count} directories, {file_count} files"),
    };
    _ = writeln!(io::stdout(), "{summary}");

    Ok(())
}

/// Computes each entry's cumulative apparent size (its own stat size plus,
/// for directories, everything beneath it), keyed by path — the same
/// accounting as `tree --du`. Returns the map and the grand total of the
/// top-level entries.
///
/// Entries must be in depth-first order: a forward pass records each
/// entry's parent index, then a reverse pass rolls sizes up, so the whole
/// computation is O(n) with one `stat` per entry.
fn compute_cumulative_sizes(
    entries: &[ignore::DirEntry],
) -> (std::collections::HashMap<std::path::PathBuf, u64>, u64) {
    let mut sizes: Vec<u64> =
        entries.iter().map(|e| e.metadata().ok().map(|m| m.len()).unwrap_or(0)).collect();

    let mut parent_index: Vec<Option<usize>> = vec![None; entries.len()];
    let mut ancestors: Vec<usize> = Vec::new(); // ancestors[k] is at depth k + 1
    for (index, entry) in entries.iter().enumerate() {
        ancestors.truncate(entry.depth().saturating_sub(1));
        parent_index[index] = ancestors.last().copied();
        ancestors.push(index);
    }

    for index in (0..entries.len()).rev() {
        if let Some(parent) = parent_index[index] {
            sizes[parent] += sizes[index];
        }
    }

    let total = entries
        .iter()
        .zip(&sizes)
        .filter(|(entry, _)| entry.depth() == 1)
        .map(|(_, size)| size)
        .sum();
    let map = entries.iter().zip(sizes).map(|(e, s)| (e.path().to_path_buf(), s)).collect();
    (map, total)
}

/// Renders the node list as a nested JSON document (loosely modeled on
/// `tree -J`): the root object has `path`, `type`, `contents`, and a
/// `report` with the directory/file totals. Optional per-entry fields
/// (`size`, `permissions`, `git_status`) appear when the matching flags
/// are set; summary markers from the display limits become
/// `{"type": "summary", ...}` objects.
#[allow(clippy::too_many_arguments)]
fn render_json(
    args: &ViewArgs,
    nodes: &[TreeNode],
    dir_count: usize,
    file_count: usize,
    status_cache: Option<&git::StatusCache>,
    root_in_repo: Option<&std::path::PathBuf>,
    du_map: Option<&std::collections::HashMap<std::path::PathBuf, u64>>,
    du_total: Option<u64>,
) -> serde_json::Value {
    use serde_json::{json, Map, Value};

    fn build_level(
        nodes: &[TreeNode],
        index: &mut usize,
        depth: usize,
        args: &ViewArgs,
        status_cache: Option<&git::StatusCache>,
        root_in_repo: Option<&std::path::PathBuf>,
        du_map: Option<&std::collections::HashMap<std::path::PathBuf, u64>>,
    ) -> Vec<Value> {
        let mut out = Vec::new();
        while *index < nodes.len() && nodes[*index].depth() == depth {
            match &nodes[*index] {
                TreeNode::Summary { hidden_files, hidden_items, .. } => {
                    *index += 1;
                    let mut object = Map::new();
                    object.insert("type".into(), "summary".into());
                    if *hidden_files > 0 {
                        object.insert("hidden_files".into(), (*hidden_files).into());
                    }
                    if *hidden_items > 0 {
                        object.insert("hidden_items".into(), (*hidden_items).into());
                    }
                    out.push(Value::Object(object));
                }
                TreeNode::Entry(entry) => {
                    *index += 1;
                    let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());
                    let type_str = if is_dir {
                        "directory"
                    } else if entry.file_type().is_some_and(|ft| ft.is_symlink()) {
                        "symlink"
                    } else {
                        "file"
                    };

                    let mut object = Map::new();
                    object.insert(
                        "name".into(),
                        entry.file_name().to_string_lossy().into_owned().into(),
                    );
                    object.insert("type".into(), type_str.into());

                    let size_enabled = args.common.size || args.du;
                    let metadata = if size_enabled || args.common.permissions {
                        entry.metadata().ok()
                    } else {
                        None
                    };
                    if size_enabled && is_dir {
                        // Directories only carry a size under --du.
                        if let Some(size) = du_map.and_then(|map| map.get(entry.path())) {
                            object.insert("size".into(), (*size).into());
                        }
                    } else if size_enabled {
                        if let Some(md) = &metadata {
                            object.insert("size".into(), md.len().into());
                        }
                    }
                    if args.common.permissions {
                        if let Some(md) = &metadata {
                            object
                                .insert("permissions".into(), utils::permission_string(md).into());
                        }
                    }
                    if let (Some(cache), Some(base)) = (status_cache, root_in_repo) {
                        if let Some(status) = entry
                            .path()
                            .strip_prefix(&args.common.path)
                            .ok()
                            .and_then(|rel| cache.get(&base.join(rel)))
                        {
                            object
                                .insert("git_status".into(), status.get_char().to_string().into());
                        }
                    }
                    if is_dir {
                        let children = build_level(
                            nodes,
                            index,
                            depth + 1,
                            args,
                            status_cache,
                            root_in_repo,
                            du_map,
                        );
                        object.insert("contents".into(), Value::Array(children));
                    }
                    out.push(Value::Object(object));
                }
            }
        }
        out
    }

    let mut index = 0;
    let contents = build_level(nodes, &mut index, 1, args, status_cache, root_in_repo, du_map);
    let mut report = Map::new();
    report.insert("directories".into(), dir_count.into());
    report.insert("files".into(), file_count.into());
    if let Some(total) = du_total {
        report.insert("total_size".into(), total.into());
    }
    json!({
        "path": args.common.path.display().to_string(),
        "type": "directory",
        "contents": contents,
        "report": report,
    })
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
