//! Implements the interactive terminal user interface (TUI) mode.
//!
//! This module contains all logic for running `lstr` in an interactive
//! session, including state management, event handling, and rendering.

use crate::app::InteractiveArgs;
use crate::git::{self, StatusCache};
use crate::icons;
use crate::sort;
use crate::utils;
use ignore::WalkBuilder;
use lscolors::{Color as LsColor, LsColors, Style as LsStyle};
use ratatui::crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::env;
use std::fs;
use std::io::{stderr, stdout, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

// Platform-specific import for unix permissions
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Converts an lscolors::Style to a ratatui::style::Style
fn to_ratatui_style(ls_style: LsStyle) -> Style {
    let mut style = Style::default();

    if let Some(fg) = ls_style.foreground {
        style = style.fg(match fg {
            LsColor::Black => Color::Black,
            LsColor::Red => Color::Red,
            LsColor::Green => Color::Green,
            LsColor::Yellow => Color::Yellow,
            LsColor::Blue => Color::Blue,
            LsColor::Magenta => Color::Magenta,
            LsColor::Cyan => Color::Cyan,
            LsColor::White => Color::White,
            LsColor::BrightBlack => Color::Gray,
            LsColor::BrightRed => Color::LightRed,
            LsColor::BrightGreen => Color::LightGreen,
            LsColor::BrightYellow => Color::LightYellow,
            LsColor::BrightBlue => Color::LightBlue,
            LsColor::BrightMagenta => Color::LightMagenta,
            LsColor::BrightCyan => Color::LightCyan,
            LsColor::BrightWhite => Color::White,
            LsColor::Fixed(n) => Color::Indexed(n),
            LsColor::RGB(r, g, b) => Color::Rgb(r, g, b),
        });
    }

    if ls_style.font_style.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if ls_style.font_style.italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if ls_style.font_style.underline {
        style = style.add_modifier(Modifier::UNDERLINED);
    }

    style
}

enum PostExitAction {
    None,
    OpenFile(PathBuf),
    PrintPath(PathBuf),
}

#[derive(Debug, Clone)]
struct FileEntry {
    path: PathBuf,
    depth: usize,
    is_dir: bool,
    is_expanded: bool,
    size: Option<u64>,
    permissions: Option<String>,
    git_status: Option<git::FileStatus>,
}

/// Represents the current search mode of the TUI
#[derive(Debug, Clone, PartialEq)]
enum SearchMode {
    /// No search active, showing all entries
    None,
    /// Search mode activated with '/' key
    Search,
}

struct AppState {
    master_entries: Vec<FileEntry>,
    visible_entries: Vec<FileEntry>,
    list_state: ListState,
    /// Current search/filter mode
    search_mode: SearchMode,
    /// Current search query string
    search_query: String,
    /// Backup of visible entries before search/filter was applied
    original_visible_entries: Vec<FileEntry>,
}

impl AppState {
    fn new(args: &InteractiveArgs, root_path: &Path) -> anyhow::Result<Self> {
        let git_repo_status = if args.git_status { git::load_status(root_path)? } else { None };

        let status_info = git_repo_status.as_ref().map(|s| (&s.cache, &s.root));
        let mut master_entries = scan_directory(root_path, status_info, args)?;

        if let Some(expand_level) = args.expand_level {
            for entry in &mut master_entries {
                if entry.is_dir && entry.depth < expand_level {
                    entry.is_expanded = true;
                }
            }
        }

        let mut app_state = Self {
            master_entries,
            visible_entries: Vec::new(),
            list_state: ListState::default(),
            search_mode: SearchMode::None,
            search_query: String::new(),
            original_visible_entries: Vec::new(),
        };
        app_state.regenerate_visible_entries();
        if !app_state.visible_entries.is_empty() {
            app_state.list_state.select(Some(0));
        }
        Ok(app_state)
    }

    fn regenerate_visible_entries(&mut self) {
        self.visible_entries.clear();
        let mut parent_expanded_stack: Vec<bool> = Vec::new();
        for entry in &self.master_entries {
            while parent_expanded_stack.len() >= entry.depth {
                parent_expanded_stack.pop();
            }
            if parent_expanded_stack.iter().all(|&x| x) {
                self.visible_entries.push(entry.clone());
            }
            if entry.is_dir {
                parent_expanded_stack.push(entry.is_expanded);
            }
        }
    }

    fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.visible_entries.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.visible_entries.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn get_selected_entry(&self) -> Option<&FileEntry> {
        self.list_state.selected().and_then(|i| self.visible_entries.get(i))
    }

    fn toggle_selected_directory(&mut self) {
        if let Some(selected_index) = self.list_state.selected() {
            let selected_path = self.visible_entries[selected_index].path.clone();
            if let Some(master_entry) =
                self.master_entries.iter_mut().find(|e| e.path == selected_path)
            {
                if master_entry.is_dir {
                    master_entry.is_expanded = !master_entry.is_expanded;
                }
            }
            self.regenerate_visible_entries();
            if let Some(new_index) =
                self.visible_entries.iter().position(|e| e.path == selected_path)
            {
                self.list_state.select(Some(new_index));
            } else {
                let new_selection = selected_index.min(self.visible_entries.len() - 1);
                self.list_state.select(Some(new_selection));
            }
        }
    }

    /// Enter search mode (activated by '/' key)
    fn enter_search_mode(&mut self) {
        if self.search_mode == SearchMode::None {
            self.original_visible_entries = self.visible_entries.clone();
        }
        self.search_mode = SearchMode::Search;
        self.search_query.clear();
    }

    /// Exit search/filter mode and restore original view
    fn exit_search_mode(&mut self) {
        if self.search_mode != SearchMode::None {
            self.visible_entries = self.original_visible_entries.clone();
            self.original_visible_entries.clear();
            self.search_mode = SearchMode::None;
            self.search_query.clear();

            // Restore selection to valid index
            if let Some(selected) = self.list_state.selected() {
                if selected >= self.visible_entries.len() {
                    let new_selection = if self.visible_entries.is_empty() {
                        None
                    } else {
                        Some(self.visible_entries.len() - 1)
                    };
                    self.list_state.select(new_selection);
                }
            }
        }
    }

    /// Check if currently in any search/filter mode
    fn in_search_mode(&self) -> bool {
        self.search_mode != SearchMode::None
    }

    /// Append character to search query and apply filter
    fn append_to_query(&mut self, c: char) {
        if self.search_mode != SearchMode::None {
            self.search_query.push(c);
            self.apply_search_filter();
        }
    }

    /// Remove last character from search query and apply filter
    fn remove_from_query(&mut self) {
        if self.search_mode != SearchMode::None && !self.search_query.is_empty() {
            self.search_query.pop();
            self.apply_search_filter();
        }
    }

    /// Apply current search query to filter visible entries
    fn apply_search_filter(&mut self) {
        if self.search_mode == SearchMode::None || self.search_query.is_empty() {
            // If no search or empty query, show original entries
            self.visible_entries = self.original_visible_entries.clone();
        } else {
            // Filter entries based on search query (case-insensitive filename match)
            let query_lower = self.search_query.to_lowercase();
            self.visible_entries = self
                .original_visible_entries
                .iter()
                .filter(|entry| {
                    entry
                        .path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
        }

        // Reset selection to first item if current selection is out of bounds
        if let Some(selected) = self.list_state.selected() {
            if selected >= self.visible_entries.len() {
                let new_selection = if self.visible_entries.is_empty() { None } else { Some(0) };
                self.list_state.select(new_selection);
            }
        }
    }
}

pub fn run(args: &InteractiveArgs, ls_colors: &LsColors) -> anyhow::Result<()> {
    if !args.path.is_dir() {
        anyhow::bail!("'{}' is not a directory.", args.path.display());
    }
    let root_path = fs::canonicalize(&args.path)?;

    let mut app_state = AppState::new(args, &root_path)?;
    let mut terminal = setup_terminal()?;
    let post_exit_action = run_app(&mut terminal, &mut app_state, args, ls_colors)?;
    restore_terminal(&mut terminal)?;

    match post_exit_action {
        PostExitAction::OpenFile(path) => {
            let editor = env::var("EDITOR").unwrap_or_else(|_| {
                if cfg!(windows) {
                    "notepad".to_string()
                } else {
                    "vim".to_string()
                }
            });
            Command::new(editor).arg(path).status()?;
        }
        PostExitAction::PrintPath(path) => {
            let path_string = path.display().to_string();
            let display_path = if path_string.len() <= 260 {
                path_string.replace(r"\\?\", "")
            } else {
                // Keep \\?\ when it exceeds 260 characters. to avoid potential problems.
                path_string
            };
            println!("{}", display_path);
        }
        PostExitAction::None => {}
    }

    Ok(())
}

fn run_app<B: Backend + Write>(
    terminal: &mut Terminal<B>,
    app_state: &mut AppState,
    args: &InteractiveArgs,
    ls_colors: &LsColors,
) -> anyhow::Result<PostExitAction> {
    loop {
        terminal.draw(|f| ui(f, app_state, args, ls_colors))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('s') if key.modifiers == KeyModifiers::CONTROL => {
                        if let Some(entry) = app_state.get_selected_entry() {
                            break Ok(PostExitAction::PrintPath(entry.path.clone()));
                        }
                    }
                    KeyCode::Char('q') => {
                        break Ok(PostExitAction::None);
                    }
                    KeyCode::Esc => {
                        if app_state.in_search_mode() {
                            app_state.exit_search_mode();
                        } else {
                            break Ok(PostExitAction::None);
                        }
                    }
                    KeyCode::Char('/') if !app_state.in_search_mode() => {
                        app_state.enter_search_mode();
                    }
                    KeyCode::Backspace if app_state.in_search_mode() => {
                        app_state.remove_from_query();
                    }
                    KeyCode::Char(c)
                        if app_state.in_search_mode()
                            && (c.is_alphanumeric()
                                || c == '.'
                                || c == '_'
                                || c == '-'
                                || c == ' ') =>
                    {
                        app_state.append_to_query(c);
                    }
                    KeyCode::Down | KeyCode::Char('j') => app_state.next(),
                    KeyCode::Up | KeyCode::Char('k') => app_state.previous(),
                    KeyCode::Enter => {
                        if let Some(entry) = app_state.get_selected_entry() {
                            if entry.is_dir {
                                app_state.toggle_selected_directory();
                            } else {
                                break Ok(PostExitAction::OpenFile(entry.path.clone()));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app_state: &mut AppState, args: &InteractiveArgs, ls_colors: &LsColors) {
    let frame_width = f.size().width as usize;
    let items: Vec<ListItem> = app_state
        .visible_entries
        .iter()
        .map(|entry| {
            let mut spans = Vec::new();
            if args.git_status {
                let (status_char, status_color) = if let Some(status) = entry.git_status {
                    let color = match status {
                        git::FileStatus::New | git::FileStatus::Renamed => Color::Green,
                        git::FileStatus::Modified | git::FileStatus::Typechange => Color::Yellow,
                        git::FileStatus::Deleted => Color::Red,
                        git::FileStatus::Conflicted => Color::LightRed,
                        git::FileStatus::Untracked => Color::Magenta,
                    };
                    (status.get_char().to_string(), color)
                } else {
                    (" ".to_string(), Color::Reset)
                };
                spans.push(Span::styled(
                    format!("{status_char} "),
                    Style::default().fg(status_color),
                ));
            }
            if args.permissions {
                let perms_str = entry.permissions.as_deref().unwrap_or("----------");
                spans.push(Span::styled(
                    format!("{perms_str} "),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            let indent_str = "    ".repeat(entry.depth.saturating_sub(1));
            spans.push(Span::raw(indent_str));
            let branch_str = if entry.is_dir {
                if entry.is_expanded {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };
            spans.push(Span::raw(branch_str));
            if args.icons {
                let (icon, color) = icons::get_icon_for_path(&entry.path, entry.is_dir);
                spans.push(Span::styled(format!("{icon} "), Style::default().fg(map_color(color))));
            }

            let name = entry.path.file_name().unwrap().to_string_lossy();
            let lscolors_style = ls_colors.style_for_path(&entry.path).cloned().unwrap_or_default();
            let ratatui_style = to_ratatui_style(lscolors_style);
            let name_span = Span::styled(name.to_string(), ratatui_style);
            spans.push(name_span);

            if args.size && !entry.is_dir {
                if let Some(size) = entry.size {
                    let size_str = utils::format_size(size);
                    let left_len: usize = spans.iter().map(|s| s.width()).sum();
                    let padding =
                        frame_width.saturating_sub(left_len).saturating_sub(size_str.len());
                    spans.push(Span::raw(" ".repeat(padding)));
                    spans.push(Span::styled(size_str, Style::default().fg(Color::DarkGray)));
                }
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    // Create layout: main area for list + bottom line for status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Main area (flexible)
            Constraint::Length(1), // Status line (1 row)
        ])
        .split(f.size());

    // Render the file list in the main area
    let list = List::new(items)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");
    f.render_stateful_widget(list, chunks[0], &mut app_state.list_state);

    // Create and render status line
    let status_text = if app_state.in_search_mode() {
        let match_count = app_state.visible_entries.len();
        format!("Search: {} ({} matches)", app_state.search_query, match_count)
    } else {
        // Show help text when not searching
        "Press / to search, q to quit".to_string()
    };

    let status_paragraph = Paragraph::new(status_text).style(if app_state.in_search_mode() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    });
    f.render_widget(status_paragraph, chunks[1]);
}

fn scan_directory(
    path: &Path,
    status_info: Option<(&StatusCache, &PathBuf)>,
    args: &InteractiveArgs,
) -> anyhow::Result<Vec<FileEntry>> {
    let mut builder = WalkBuilder::new(path);
    builder.hidden(!args.all).git_ignore(args.gitignore);

    // Collect all DirEntry objects first, filtering out the root path
    let mut dir_entries: Vec<_> =
        builder.build().flatten().filter(|result| result.path() != path).collect();

    // Apply tree-aware sorting to preserve parent-child relationships
    let sort_options = args.to_sort_options();
    sort::sort_entries_hierarchically(&mut dir_entries, &sort_options);

    // Convert DirEntry objects to FileEntry objects
    let mut entries = Vec::new();
    for result in dir_entries {
        let metadata = if args.size || args.permissions { result.metadata().ok() } else { None };
        let is_dir = result.file_type().is_some_and(|ft| ft.is_dir());
        let git_status = if let Some((cache, root)) = status_info {
            result.path().strip_prefix(root).ok().and_then(|rel_path| cache.get(rel_path)).copied()
        } else {
            None
        };
        let size = if args.size && !is_dir { metadata.as_ref().map(|m| m.len()) } else { None };
        let permissions = if args.permissions {
            metadata.map(|_md| {
                #[cfg(unix)]
                {
                    let mode = _md.permissions().mode();
                    let file_type_char = if _md.is_dir() { 'd' } else { '-' };
                    format!("{}{}", file_type_char, utils::format_permissions(mode))
                }
                #[cfg(not(unix))]
                {
                    "----------".to_string()
                }
            })
        } else {
            None
        };
        entries.push(FileEntry {
            path: result.path().to_path_buf(),
            depth: result.depth(),
            is_dir,
            is_expanded: false,
            size,
            permissions,
            git_status,
        });
    }
    Ok(entries)
}

fn map_color(c: colored::Color) -> Color {
    match c {
        colored::Color::Black => Color::Black,
        colored::Color::Red => Color::Red,
        colored::Color::Green => Color::Green,
        colored::Color::Yellow => Color::Yellow,
        colored::Color::Blue => Color::Blue,
        colored::Color::Magenta => Color::Magenta,
        colored::Color::Cyan => Color::Cyan,
        colored::Color::White => Color::White,
        colored::Color::BrightBlack => Color::Gray,
        colored::Color::BrightRed => Color::LightRed,
        colored::Color::BrightGreen => Color::LightGreen,
        colored::Color::BrightYellow => Color::LightYellow,
        colored::Color::BrightBlue => Color::LightBlue,
        colored::Color::BrightMagenta => Color::LightMagenta,
        colored::Color::BrightCyan => Color::LightCyan,
        colored::Color::TrueColor { r, g, b } => Color::Rgb(r, g, b),
        _ => Color::Reset,
    }
}

type TerminalWriter = CrosstermBackend<Box<dyn Write + Send>>;

fn setup_terminal() -> anyhow::Result<Terminal<TerminalWriter>> {
    let writer: Box<dyn Write + Send> =
        if stdout().is_terminal() { Box::new(stdout()) } else { Box::new(stderr()) };
    enable_raw_mode()?;
    let mut writer_mut = writer;
    execute!(writer_mut, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(writer_mut);
    Terminal::new(backend).map_err(anyhow::Error::from)
}

fn restore_terminal<B: Backend + Write>(terminal: &mut Terminal<B>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup_test_app_state() -> AppState {
        let master_entries = vec![
            FileEntry {
                path: PathBuf::from("src"),
                depth: 1,
                is_dir: true,
                is_expanded: false,
                size: None,
                permissions: Some("drwxr-xr-x".to_string()),
                git_status: None,
            },
            FileEntry {
                path: PathBuf::from("src/main.rs"),
                depth: 2,
                is_dir: false,
                is_expanded: false,
                size: Some(1024),
                permissions: Some("-rw-r--r--".to_string()),
                git_status: Some(git::FileStatus::Modified),
            },
            FileEntry {
                path: PathBuf::from("README.md"),
                depth: 1,
                is_dir: false,
                is_expanded: false,
                size: Some(512),
                permissions: Some("-rw-r--r--".to_string()),
                git_status: None,
            },
        ];
        let mut app_state = AppState {
            master_entries,
            visible_entries: Vec::new(),
            list_state: ListState::default(),
            search_mode: SearchMode::None,
            search_query: String::new(),
            original_visible_entries: Vec::new(),
        };
        app_state.regenerate_visible_entries();
        app_state.list_state.select(Some(0));
        app_state
    }
    #[test]
    fn test_navigation() {
        let mut app_state = setup_test_app_state();
        assert_eq!(app_state.list_state.selected(), Some(0));
        app_state.next();
        assert_eq!(app_state.list_state.selected(), Some(1));
        app_state.next();
        assert_eq!(app_state.list_state.selected(), Some(0));
        app_state.previous();
        assert_eq!(app_state.list_state.selected(), Some(1));
        app_state.previous();
        assert_eq!(app_state.list_state.selected(), Some(0));
    }
    #[test]
    fn test_toggle_directory() {
        let mut app_state = setup_test_app_state();
        assert_eq!(app_state.visible_entries.len(), 2);
        app_state.list_state.select(Some(0));
        app_state.toggle_selected_directory();
        assert_eq!(app_state.visible_entries.len(), 3);
        assert_eq!(app_state.visible_entries[1].path, PathBuf::from("src/main.rs"));
        app_state.toggle_selected_directory();
        assert_eq!(app_state.visible_entries.len(), 2);
    }
    #[test]
    fn test_get_selected_entry() {
        let mut app_state = setup_test_app_state();
        app_state.list_state.select(Some(1));
        let selected = app_state.get_selected_entry();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().path, PathBuf::from("README.md"));
    }
}
