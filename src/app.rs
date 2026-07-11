//! Defines the command-line interface for the lstr application.

use crate::sort;
use clap::{Parser, Subcommand, ValueEnum};
use std::fmt;
use std::path::PathBuf;

/// A blazingly fast, minimalist directory tree viewer, written in Rust.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
#[command(override_usage = "lstr [OPTIONS] [PATH]\n    lstr interactive [OPTIONS] [PATH]")]
pub struct Args {
    /// The subcommand to run. If no subcommand is specified, the classic tree view is displayed.
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// The arguments for the classic tree view. These are used when no subcommand is provided.
    #[command(flatten)]
    pub view: ViewArgs,
}

/// Defines the available subcommands for the application.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the interactive TUI explorer.
    #[command(visible_alias = "i")]
    Interactive(InteractiveArgs),
}

/// Arguments shared by the classic tree view and the interactive TUI.
#[derive(Parser, Debug, Default)]
pub struct CommonArgs {
    /// The path to the directory to display. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Show all files, including hidden ones.
    #[arg(short = 'a', long)]
    pub all: bool,
    /// Respect .gitignore and other standard ignore files.
    #[arg(short = 'g', long)]
    pub gitignore: bool,
    /// Show git status for files and directories.
    #[arg(short = 'G', long)]
    pub git_status: bool,
    /// Display file-specific icons (requires a Nerd Font).
    #[arg(long)]
    pub icons: bool,
    /// Display the size of files.
    #[arg(short = 's', long)]
    pub size: bool,
    /// Display file permissions.
    #[arg(short = 'p', long)]
    pub permissions: bool,
    /// Sort entries by the specified criteria.
    #[arg(long, default_value_t = sort::SortType::Name)]
    pub sort: sort::SortType,
    /// Sort directories before files.
    #[arg(long)]
    pub dirs_first: bool,
    /// Use case-sensitive sorting.
    #[arg(long)]
    pub case_sensitive: bool,
    /// Use natural/version sorting (e.g., file1 < file10).
    #[arg(long)]
    pub natural_sort: bool,
    /// Reverse the sort order.
    #[arg(short = 'r', long)]
    pub reverse: bool,
    /// Sort dotfiles and dotfolders first.
    #[arg(long)]
    pub dotfiles_first: bool,
}

impl CommonArgs {
    /// Creates a SortOptions instance from the shared sorting flags.
    pub fn to_sort_options(&self) -> sort::SortOptions {
        sort::SortOptions {
            sort_type: self.sort,
            directories_first: self.dirs_first,
            case_sensitive: self.case_sensitive,
            natural_sort: self.natural_sort,
            reverse: self.reverse,
            dotfiles_first: self.dotfiles_first,
        }
    }
}

/// Arguments for the classic `view` command.
#[derive(Parser, Debug, Default)]
pub struct ViewArgs {
    /// The arguments shared with the interactive TUI.
    #[command(flatten)]
    pub common: CommonArgs,
    /// Specify when to use colorized output.
    #[arg(long, value_name = "WHEN", default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,
    /// Maximum depth to descend in the directory tree.
    #[arg(short = 'L', long)]
    pub level: Option<usize>,
    /// Display directories only.
    #[arg(short = 'd', long)]
    pub dirs_only: bool,
    /// Render file paths as clickable hyperlinks.
    #[arg(long)]
    pub hyperlinks: bool,
}

/// Arguments for the `interactive` command.
#[derive(Parser, Debug, Default)]
pub struct InteractiveArgs {
    /// The arguments shared with the classic tree view.
    #[command(flatten)]
    pub common: CommonArgs,
    /// Initial depth to expand the directory tree.
    #[arg(long, value_name = "LEVEL")]
    pub expand_level: Option<usize>,
    /// Command used to open files, overriding $VISUAL and $EDITOR.
    /// Split on whitespace; the file path is appended as the last argument.
    #[arg(long, value_name = "COMMAND")]
    pub editor: Option<String>,
}

/// Defines the choices for the --color option.
#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ColorChoice {
    Always,
    #[default]
    Auto,
    Never,
}

/// Implements the Display trait for ColorChoice to show possible values in help messages.
impl fmt::Display for ColorChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_possible_value().expect("no values are skipped").get_name().fmt(f)
    }
}
