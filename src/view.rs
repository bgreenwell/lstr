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

    // Build tree structure information
    let tree_info = build_tree_info(&entries);

    for (index, entry) in entries.iter().enumerate() {
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

        let (prefix, connector) = &tree_info[index];
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

        let final_name = if args.hyperlinks && !is_dir {
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

        if is_dir {
            dir_count += 1;
        } else {
            file_count += 1;
        }

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

/// Builds tree structure information for proper connector display.
/// Returns one (prefix, connector) pair per entry, in entry order.
///
/// Entries must be in depth-first order (each entry's parent precedes it),
/// which `sort_entries_hierarchically` guarantees. Under that invariant an
/// entry is the last of its siblings exactly when the next entry at a depth
/// less than or equal to its own is strictly shallower, so everything can be
/// computed in two linear passes instead of rescanning the list per entry.
fn build_tree_info(entries: &[ignore::DirEntry]) -> Vec<(String, &'static str)> {
    // Reverse pass: pending[d] is true when a later entry at depth d exists
    // within the same parent scope (deeper flags are cleared whenever a
    // shallower entry is seen, since those entries belong to its subtree).
    let mut is_last = vec![false; entries.len()];
    let mut pending: Vec<bool> = Vec::new();
    for (index, entry) in entries.iter().enumerate().rev() {
        let depth = entry.depth();
        if pending.len() <= depth {
            pending.resize(depth + 1, false);
        } else {
            pending.truncate(depth + 1);
        }
        is_last[index] = !pending[depth];
        pending[depth] = true;
    }

    // Forward pass: build each entry's prefix from its ancestors' last-sibling
    // flags, maintained as a stack indexed by depth.
    let mut tree_info = Vec::with_capacity(entries.len());
    let mut prefix_parts: Vec<&'static str> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let depth = entry.depth();
        prefix_parts.truncate(depth.saturating_sub(1));
        let prefix = prefix_parts.concat();
        let connector = if is_last[index] { "└──" } else { "├──" };
        // How this entry contributes to its descendants' prefixes.
        prefix_parts.push(if is_last[index] { "    " } else { "│   " });
        tree_info.push((prefix, connector));
    }

    tree_info
}
