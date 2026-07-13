# lstr

[![CI](https://img.shields.io/github/actions/workflow/status/bgreenwell/lstr/ci.yml?style=for-the-badge)](https://github.com/bgreenwell/lstr/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/lstr.svg?style=for-the-badge&color=%234E9A06)](https://crates.io/crates/lstr)
[![Downloads](https://img.shields.io/crates/d/lstr?style=for-the-badge&color=%234E9A06)](https://crates.io/crates/lstr)

[![License: MIT](https://img.shields.io/badge/License-MIT-%232196F3.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-%23D34516.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Easy Install](https://img.shields.io/badge/Easy%20Install-Homebrew%20%7C%20Scoop%20%7C%20WinGet-%23FBB040?style=for-the-badge)](#installation)
[![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS%20%7C%20Windows-blue?style=for-the-badge)](https://github.com/bgreenwell/lstr/releases/latest)

A fast, minimalist directory tree viewer, written in Rust. Inspired by the command line program [tree](https://github.com/Old-Man-Programmer/tree), with a powerful interactive mode.

![](assets/lstr-demo.gif)

*An interactive overview of a project's structure using `lstr`.*

## Philosophy

  - **Minimalist:** Provides essential features without the bloat. The core experience is clean and uncluttered.
  - **Interactive:** An optional TUI mode for fluid, keyboard-driven exploration.

## Features

  - **Classic and interactive modes:** Use `lstr` for a classic `tree`-like view, or launch `lstr interactive` for a fully interactive TUI with keyboard and mouse navigation, filename search (`/`), and in-place file opening that returns to the tree when your editor exits.
  - **Theme-aware coloring:** Respects your system's `LS_COLORS` environment variable for fully customizable file and directory colors.
  - **Rich information display (optional):**
      - Display file-specific icons with `--icons` (requires a Nerd Font).
      - Show file permissions with `-p`, including symlink and setuid/setgid/sticky indicators.
      - Show file sizes with `-s`, or cumulative directory sizes with `--du`.
      - **Git integration:** Show file statuses (`M`, `A`, `?`, etc.) with `-G`; directories reflect the status of their contents.
  - **Flexible sorting:** By name, size, modification time, or extension, with directories-first, natural/version, case-sensitive, and reverse variants.
  - **Smart filtering:**
      - Respects your `.gitignore` files with the `-g` flag.
      - Control recursion depth (`-L`) or show only directories (`-d`).
      - Summarize deep or crowded directories with `--file-depth` and `--max-items`.
  - **Scriptable:** JSON output (`--output json`), a self-contained HTML directory index (`--output html`), pipe-friendly text, and a `Ctrl+s` file-picker mode for shell integration.

## Installation

### With Homebrew (macOS)

The easiest way to install `lstr` on macOS is with Homebrew.

```zsh
brew install lstr
```

### Arch Linux (AUR)

A community-maintained [`lstr-git`](https://aur.archlinux.org/packages/lstr-git) package (thanks, [@bakatrouble](https://github.com/bakatrouble)!) builds the latest commit from this repository. Install it with your preferred AUR helper, e.g.:

```bash
paru -S lstr-git
```

### From source (all platforms)

You need the Rust toolchain installed on your system to build `lstr`.

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/bgreenwell/lstr.git
    cd lstr
    ```
2.  **Build and install using Cargo:**
    ```bash
    cargo install --path .
    ```

### NetBSD

On NetBSD a package is available from the official repositories. To install it, simply run:

```bash
pkgin install lstr
```

## Usage

```bash
lstr [OPTIONS] [PATH]
lstr interactive [OPTIONS] [PATH]
```

Note that `PATH` defaults to the current directory (`.`) if not specified.

| Option                 | Description                                                                 |
| :--------------------- | :-------------------------------------------------------------------------- |
| `-a`, `--all`          | List all files and directories, including hidden ones.                      |
| `--color <WHEN>`       | Specify when to use color output (`always`, `auto`, `never`).               |
| `-d`, `--dirs-only`    | List directories only, ignoring all files.                                  |
| `-g`, `--gitignore`    | Respect `.gitignore` and other standard ignore files.                       |
| `-G`, `--git-status`   | Show git status for files and directories.                                  |
| `--icons`              | Display file-specific icons; requires a [Nerd Font](https://www.nerdfonts.com/). |
| `--hyperlinks`         | Render file paths as clickable hyperlinks (classic mode only)               |
| `-L`, `--level <LEVEL>`| Maximum depth to descend.                                                   |
| `--file-depth <LEVEL>` | Hide individual files below this depth, summarized as `[+N files]` (classic mode only). |
| `--max-items <N>`      | Show at most N entries per directory, summarized as `[+N more]` (classic mode only). |
| `--du`                 | Show directories with the cumulative size of their contents, like `tree --du`; implies `-s` (classic mode only). |
| `--output <FORMAT>`    | Output format: `text` (default), `json`, or `html` (classic mode only).     |
| `-p`, `--permissions`  | Display file permissions (Unix-like systems only).                          |
| `-s`, `--size`         | Display the size of files.                                                  |
| `--sort <TYPE>`        | Sort entries by the specified criteria (`name`, `size`, `modified`, `extension`). |
| `--dirs-first`         | Sort directories before files.                                              |
| `--case-sensitive`     | Use case-sensitive sorting.                                                 |
| `--natural-sort`       | Use natural/version sorting (e.g., file1 < file10). Takes precedence over `--case-sensitive`. |
| `-r`, `--reverse`      | Reverse the sort order.                                                     |
| `--dotfiles-first`     | Sort dotfiles and dotfolders first (dotfolders → folders → dotfiles → files). Implies `--dirs-first`. |
| `--expand-level <LEVEL>`| **Interactive mode only:** Initial depth to expand the interactive tree.   |
| `--editor <COMMAND>`   | **Interactive mode only:** Command used to open files, overriding `$VISUAL`/`$EDITOR`. |

-----

## Output formats

Besides the default tree view, `lstr` can emit JSON for scripting or a
self-contained HTML directory index for browsing offline. Directories
render as collapsible `<details>` elements and files as relative links, so
the page can be saved next to the tree it describes and opened directly in
a browser; `-s`, `-p`, and `-G` add size, permissions, and git-status
annotations, same as the other output formats. This is the live output of
`lstr --output html -s assets` on [`examples/sample-directory/assets`](examples/sample-directory/assets)
(GitHub strips the `<style>` the real output ships with, so it renders
plainer here than it does in an actual browser):

<ul>
<li><details open><summary>data</summary>
<ul>
<li><a href="examples/sample-directory/assets/data/config.yaml">config.yaml</a> (153 B)</li>
<li><a href="examples/sample-directory/assets/data/database.sqlite">database.sqlite</a> (54 B)</li>
<li><a href="examples/sample-directory/assets/data/sample.csv">sample.csv</a> (87 B)</li>
</ul>
</details></li>
<li><details open><summary>fonts</summary>
<ul>
<li><a href="examples/sample-directory/assets/fonts/bold.woff2">bold.woff2</a> (58 B)</li>
<li><a href="examples/sample-directory/assets/fonts/regular.ttf">regular.ttf</a> (56 B)</li>
</ul>
</details></li>
<li><details open><summary>images</summary>
<ul>
<li><a href="examples/sample-directory/assets/images/banner.jpg">banner.jpg</a> (58 B)</li>
<li><a href="examples/sample-directory/assets/images/favicon.ico">favicon.ico</a> (56 B)</li>
<li><a href="examples/sample-directory/assets/images/logo.png">logo.png</a> (57 B)</li>
</ul>
</details></li>
</ul>

<sub>3 directories, 8 files</sub>

-----

## Interactive mode

Launch the TUI with `lstr interactive [OPTIONS] [PATH]`.

### Keyboard controls

| Key(s)  | Action                                                                                                                                      |
| :------ | :------------------------------------------------------------------------------------------------------------------------------------------ |
| `↑` / `k` | Move selection up. |
| `↓` / `j` | Move selection down. |
| `←` / `h` | Collapse the selected directory, or jump to and collapse its parent. |
| `→` / `l` | Same as `Enter`. |
| `Enter` | **Context-aware action:**<br>- If on a file: Open it in the configured editor (`--editor`, `$VISUAL`, or `$EDITOR`), then return to the tree.<br>- If on a directory: Toggle expand/collapse. |
| `/` | Search: filter entries by name as you type (substring, or a glob like `*.rs` / `test_?.py` if the query contains `*` or `?`). `Esc` exits search. |
| `q` / `Esc` | Quit the application normally. |
| `Ctrl`+`s` | **Shell integration:** Quits and prints the selected path to stdout. |
| Mouse | Scroll wheel moves the selection; click selects a row; clicking the selected entry activates it (open file / toggle directory). Hold `Shift` for normal terminal text selection. |

## Examples

**1. List the contents of the current directory**

```bash
lstr
```

**2. Explore a project interactively, ignoring gitignored files**

```bash
lstr interactive -g --icons
```

**3. Display a directory with file sizes and permissions (classic view)**

```bash
lstr -sp
```

**4. See the git status of all files in a project**

```bash
lstr -aG
```

**5. Get a tree with clickable file links (in a supported terminal)**

```bash
lstr --hyperlinks
```

**6. Start an interactive session with all data displayed**

```bash
lstr interactive -gG --icons -s -p
```

**7. Sort files naturally with directories first**

```bash
lstr --dirs-first --natural-sort
```

**8. Sort by file size in descending order**

```bash
lstr --sort size --reverse
```

**9. Sort by extension with case-sensitive ordering**

```bash
lstr --sort extension --case-sensitive
```

**10. Sort with dotfiles and dotfolders first**

```bash
lstr --dotfiles-first -a
```

## Piping and shell interaction

The classic `view` mode is designed to work well with other command-line tools via pipes (`|`).

### Interactive fuzzy finding with `fzf`

This is a powerful way to instantly find any file in a large project.

```bash
lstr -a -g --icons | fzf
```

`fzf` will take the tree from `lstr` and provide an interactive search prompt to filter it.

### Paging large trees with `less` or `bat`

If a directory is too large to fit on one screen, pipe the output to a *pager*.

```bash
# Using less (the -R flag preserves color)
lstr -L 10 | less -R

# Using bat (a modern pager that understands colors)
lstr --icons | bat
```

### Changing directories with `lstr`

You can use `lstr` as a visual `cd` command. Add the following function to your shell's startup file (e.g., `~/.bashrc`, `~/.zshrc`):

```bash
# A function to visually change directories with lstr
lcd() {
    # Run lstr and capture the selected path into a variable.
    # The TUI will draw on stderr, and the final path will be on stdout.
    local selected_dir
    selected_dir="$(lstr interactive -g --icons)"

    # If the user selected a path (and didn't just quit), `cd` into it.
    # Check if the selection is a directory.
    if [[ -n "$selected_dir" && -d "$selected_dir" ]]; then
        cd "$selected_dir"
    fi
}
```

After adding this and starting a new shell session (or running `source ~/.bashrc`), you can simply run:

```bash
lcd
```

This will launch the `lstr` interactive UI. Navigate to the directory you want, press `Ctrl+s`, and your shell's current directory will instantly change.

## Color customization

`lstr` respects your terminal's color theme by default. It reads the `LS_COLORS` environment variable to colorize files and directories according to your system's configuration. This is the same variable used by GNU `ls` and other modern command-line tools.

### Linux

On most Linux distributions, this variable is already set. You can customize it by modifying your shell's startup file.

### macOS

macOS does not set the `LS_COLORS` variable by default. To enable this feature, you can install `coreutils`:

```bash
brew install coreutils
```

Then, add the following line to your shell's startup file (e.g., `~/.zshrc` or `~/.bash_profile`):

```bash
# Use gdircolors from the newly installed coreutils
eval "$(gdircolors)"
```

### Windows

Windows does not use the `LS_COLORS` variable natively, but you can set it manually to enable color support in modern terminals like Windows Terminal.

First, copy a standard `LS_COLORS` string, such as this one:
`rs=0:di=01;34:ln=01;36:ex=01;32:*.zip=01;31:*.png=01;35:`. This string defines colors for various file types:

* **Directories:** Displayed in **bold blue**.
* **Executable files:** Displayed in **bold green** (e.g., `.sh` scripts).
* **Symbolic links:** Displayed in **bold cyan**.
* **Archives:** Displayed in **bold red** (e.g., `.zip`, `.tar.gz`).
* **Image files:** Displayed in **bold magenta** (e.g., `.png`, `.jpg`).
* **Other files:** Displayed in the terminal's default text color.

To set it for your current **PowerShell** session, run:

```powershell
$env:LS_COLORS="rs=0:di=01;34:ln=01;36:ex=01;32:*.zip=01;31:*.png=01;35:"
```

To set it for your current **Command Prompt** (cmd) session, run:

```cmd
set LS_COLORS=rs=0:di=01;34:ln=01;36:ex=01;32:*.zip=01;31:*.png=01;35:
```

To make the setting permanent, you can add the command to your PowerShell profile or set it in the system's "Environment Variables" dialog.

After setting the variable and starting a new shell session, `lstr` will automatically display your configured colors.

## Inspiration

The philosophy and functionality of `lstr` are heavily inspired by the excellent C-based [tree](https://github.com/Old-Man-Programmer/tree) command line program. This project is an attempt to recreate that classic utility in modern, safe Rust.

## License

This project is licensed under the terms of the [MIT License](LICENSE).
