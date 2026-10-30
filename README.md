# mdvi

`mdvi` is a terminal markdown viewer with Vim-style navigation.

It renders markdown into a polished full-screen TUI with fast keyboard navigation, clear typography, and predictable behavior for large files.

<img width="1595" height="582" alt="Screenshot 2026-02-10 at 4 58 34 PM" src="https://github.com/user-attachments/assets/1b2da7a1-b5fb-4169-9d10-bcd97a07ae45" />

## Features

- Full-screen terminal viewer (`crossterm` + `ratatui`)
- Vim-style navigation controls
- Visible cursor and `line:column` cursor position in the status bar
- High-quality markdown rendering via `pulldown-cmark`
- Support for:
  - headings
  - lists and task lists
  - blockquotes
  - inline markdown images (`![alt](...)`) and HTML `<img ...>` tags
  - lazy image loading with immediate placeholder layout reservation
  - syntax-highlighted fenced code blocks and inline code
  - links, tables, footnotes, emphasis/strong/strikethrough
- Configurable image backend via `--image-protocol` (`auto`, `halfblocks`, `sixel`, `kitty`, `iterm2`)
- Live reload (`r`) when the file changes on disk
- Start at specific line (`--line`)
- Works with standard terminal keys (arrows, page up/down, home/end)

Notes:
- Local, `file://`, and remote `http://` / `https://` image sources are supported.
- HTML `<img>` `width` and `height` hints are used to reserve image space before lazy loading finishes.
- On macOS, terminal-native image protocols can trigger an "Allow Terminal-Initiated Display?" prompt.
  Use `--image-protocol halfblocks` if you want image rendering without that dialog.
- Fenced code blocks with language tags (for example, a block tagged `rust`) render with syntax highlighting.
- Tables use padded columns, bold headers, row separators, and Markdown left/center/right alignment.
  Long cells wrap within their columns, including explicit `<br>` breaks. Tables reflow when the
  terminal is resized; if columns would be too cramped, each row becomes a labeled record instead.
  Inline formatting is preserved, empty record values appear as `—`, and images inside cells show
  their alt text rather than interrupting the table with a full-size image.

## Install

### Homebrew (tap)

```bash
brew tap taf2/tap
brew install mdvi
```

### Update a GitHub release binary

```bash
mdvi self-update
```

Use `mdvi self-update --check` to only check for a newer release. This is for
binaries installed from a GitHub release; use Homebrew to update a Homebrew
installation.

### Nix (flakes)

```bash
nix build
./result/bin/mdvi README.md

# Or build and run directly:
nix run . -- README.md
```

The flake uses `Cargo.lock` and a pinned nixpkgs input. It provides `default` and
`mdvi` packages for Apple Silicon/Intel macOS and ARM64/x86-64 Linux.

### From source

```bash
cargo install --path .
```

### Run without install

```bash
cargo run -- README.md
```

## Usage

```bash
mdvi [OPTIONS] <PATH>
```

Examples:

```bash
mdvi docs/spec.md
mdvi --line 120 CHANGELOG.md
mdvi --image-protocol halfblocks README.md
mdvi - < docs/spec.md
```

## Navigation

- `j` / `Down`: scroll down one line
- `k` / `Up`: scroll up one line
- `d`: half-page down (less-style)
- `u`: half-page up (less-style)
- `Ctrl-d`: half-page down
- `Ctrl-u`: half-page up
- `PageDown`: full-page down
- `PageUp`: full-page up
- `Ctrl-f`: full-page down (Vim-style)
- `Ctrl-b`: full-page up (Vim-style)
- `g` / `Home`: jump to top
- `G` / `End`: jump to bottom
- `r`: reload file from disk
- `/`: start search
- `n`: next search match
- `N`: previous search match
- `?`: toggle help line
- `q`: quit

## Why Rust?

Rust is a strong fit for a serious CLI viewer:

- precise terminal control
- excellent performance for large files
- single static binary distribution
- mature ecosystem for TUI and markdown parsing

## Development

```bash
cargo test
cargo fmt
cargo clippy -- -D warnings
```

## License

MIT
