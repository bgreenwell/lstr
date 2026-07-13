# AGENTS.md

@../CLAUDE.md

Development guide for `lstr`, a directory tree viewer with a classic
`tree`-style CLI mode and an interactive TUI. Read this before making
changes; it records the architecture, invariants, and testing conventions
that are easy to break from the outside.

## Commands

```bash
cargo test test_name -- --nocapture     # single test
./scripts/validate-basic.sh             # classic-mode output vs golden baselines
./scripts/test-dual-mode.sh             # classic + TUI consistency checks
```

Branch model: `devel` is the default and integration branch; `main` tracks
releases. Never commit directly to either — branch off `devel`
(`feature/...`, `fix/...`, `chore/...`), open a PR to `devel`, and merge
after CI is green on Ubuntu, macOS, and Windows. Releases merge `devel`
into `main` and tag `vX.Y.Z` there (see `RELEASE_CHECKLIST.md`).

## Architecture

| Module     | Responsibility |
| :--------- | :------------- |
| `main.rs`  | Entry point; dispatches to `view::run` or `tui::run`. |
| `app.rs`   | clap CLI. Shared flags live in `CommonArgs`, flattened into `ViewArgs` and `InteractiveArgs` — declare a shared flag once, there. |
| `view.rs`  | Classic mode. Pipeline: walk → filter (`--dirs-only`) → sort → count → `--du` sizes → display limits (`TreeNode` list) → render (text or JSON). |
| `tui.rs`   | Interactive mode: `AppState`, `handle_key`/`handle_mouse`, terminal setup/teardown, editor suspend/resume. |
| `sort.rs`  | `sort_entries_hierarchically` (DFS-order preserving) built on decorate-sort-undecorate `SortKey`s. |
| `git.rs`   | git2 status cache keyed by repo-relative path; file statuses propagate to ancestor directories (most severe wins). |
| `utils.rs` | Shared helpers: `configure_ignore_filters`, `permission_string`, `format_size`, `display_path` (Windows verbatim paths). |
| `color.rs` | All conversions between the lscolors, colored, and ratatui color models — never map colors inline elsewhere. |
| `icons.rs` | Nerd Font icon mapping. |

## Invariants that break silently

- **Entries are in depth-first order** after `sort_entries_hierarchically`
  (every entry's parent precedes it). `build_tree_info`,
  `apply_display_limits`, `compute_cumulative_sizes`, and the JSON renderer
  all assume it. Never sort or filter in a way that reorders parents after
  children.
- **Filter before connector computation.** Anything that removes entries
  (`--dirs-only`, display limits) must happen before `build_tree_info`, or
  suppressed entries count as siblings and connectors come out wrong.
- **Both modes share behavior through `CommonArgs`,
  `utils::configure_ignore_filters`, and the git status cache.** A change
  to walking, filtering, or status lookup must be checked in both modes —
  a break in the other mode produces no error, just wrong output.
- **Terminal restore must survive every exit path.** `setup_terminal`
  returns a drop guard and installs a panic hook (once — the TUI suspends
  and resumes around editor sessions). The release profile uses
  `panic = "abort"`, so destructors never run on panic; the hook is what
  saves the user's terminal.

## Testing conventions

- TDD: add the failing unit test (`#[cfg(test)]` in the module) or CLI test
  (`tests/cli.rs`, `assert_cmd` + `tempfile`) before the fix or feature.
- Key/mouse handling is unit-testable: construct `KeyEvent`/`MouseEvent`
  and call `handle_key`/`handle_mouse` directly on an `AppState`.
- The golden baselines in `docs/baseline-outputs/` are hand-verified; a
  diff means a regression unless the change is intentional and the
  baseline is updated in the same PR.
- Windows: filenames may not contain `"` `\` `:` `*` `?` `<` `>` `|`;
  gate tests that need such names to Unix. Directory stat sizes differ per
  filesystem (4 KiB on ext4), so size assertions must tolerate units.
- TUI end-to-end testing needs a real pty. Piping keystrokes into stdin
  does not work (input queued in canonical mode is lost at the raw-mode
  switch); use `expect`, send input after a short delay, and set the pty
  size (`stty rows 24 columns 80`) — a 0-row pty rejects mouse clicks by
  design. ratatui diff-renders, so never assert on contiguous screen
  strings; assert on behavior (e.g. Ctrl+s prints the selected path).

## Conventions

- Commits: no AI signatures or Co-Authored-By trailers.
- Sort-flag precedence (documented in help text and README):
  `--dotfiles-first` implies `--dirs-first`; `--natural-sort` takes
  precedence over `--case-sensitive`; name-comparison flags do not affect
  `--sort size/modified`.
