//! lstr: A blazingly fast, minimalist directory tree viewer.
//!
//! This is the main entry point for the lstr application. It handles parsing
//! command-line arguments and dispatching to the appropriate command handler.

// Declare the modules that make up the application.
mod app;
mod color;
mod git;
mod icons;
mod sort;
mod tui;
mod utils;
mod view;

use app::{Args, Commands};
use clap::Parser;
#[cfg(windows)]
use colored::control;
use lscolors::LsColors;

/// The main function and entry point of the application.
///
/// It parses command-line arguments and executes the corresponding command.
/// If no subcommand is given, it defaults to the classic tree `view`.
///
/// # Returns
///
/// * `Ok(())` on successful execution.
/// * `Err(anyhow::Error)` if any error occurs during execution.
fn main() -> anyhow::Result<()> {
    // On Windows, explicitly try to enable ANSI support.
    // This may fail on older versions of Windows, but we ignore the error
    // and let the `colored` crate handle it gracefully.
    #[cfg(windows)]
    let _ = control::set_virtual_terminal(true);

    // Parse the command-line arguments into our Args struct.
    let args = Args::parse();

    // Create the LsColors instance from the environment
    let ls_colors = LsColors::from_env().unwrap_or_default();

    // Check if a subcommand was passed. If not, default to the `view` command.
    match &args.command {
        Some(Commands::Interactive(interactive_args)) => tui::run(interactive_args, &ls_colors),
        None => view::run(&args.view, &ls_colors),
    }
}
