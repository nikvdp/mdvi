# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- A Nix flake for building and running mdvi on macOS and Linux with locked Rust dependencies.

### Changed
- Render tables with aligned, padded columns, bold headers, row borders, Unicode-aware cell widths, and wrapped cell text instead of joining unpadded values.
- Reflow tables to the terminal width on resize, switching cramped tables to labeled records while preserving search and image caption positions.

## [0.6.3] - 2026-02-13

### Changed
- Cached search-highlighted lines so unchanged redraws reuse prior highlighted output instead of re-running regex highlighting across the whole document.

## [0.6.2] - 2026-02-13

### Changed
- Moved local image dimension probing off the UI thread during load/reload by sending probe jobs to the background image worker.
- Removed per-frame document line cloning before paragraph rendering by transferring ownership of display lines into `Text`.
- Kept scrolling/input responsive while images load by throttling image-load result processing per UI tick.
- Moved image resize/encode work off the UI thread into a dedicated background resize worker so rendering large images no longer blocks navigation.

## [0.6.1] - 2026-02-13

### Changed
- Cached virtual-row counts in `max_scroll` keyed by content width, with invalidation on reload, search-state changes, and image layout updates to avoid rebuilding the full display document on frequent navigation/redraw paths.

## [0.6.0] - 2026-02-11

### Added
- Less-style half-page navigation aliases: `d` (down) and `u` (up), in addition to `Ctrl-d` / `Ctrl-u` (issue #1).

## [0.5.0] - 2026-02-10

### Added
- Lazy image loading with non-blocking background fetch/decode so markdown opens immediately.

### Changed
- Image layout now reserves space before load using HTML `<img width height>` hints when available.
- Images without explicit dimensions use a default placeholder estimate during lazy loading.
- Improved image rendering stability while scrolling past images by fixing partial-viewport draw artifacts and distortion.
- Long image source captions are truncated in display text to reduce wrap bleed in the viewport.

## [0.4.0] - 2026-02-10

### Added
- Visible in-document cursor with `line:column` cursor position in the status bar for easier eye tracking.

## [0.3.0] - 2026-02-10

### Added
- Syntax highlighting for fenced code blocks with recognized language tags.

## [0.2.0] - 2026-02-10

### Added
- Inline terminal image rendering for markdown images and HTML `<img ...>` tags.
- Remote image loading for `http://` and `https://` image URLs.
- `--image-protocol` CLI option to force a specific image backend (`auto`, `halfblocks`, `sixel`, `kitty`, `iterm2`).

### Changed
- Markdown image tokens now include visual fallback caption lines when an image cannot be loaded.
- TUI redraw loop now renders on input/resize state changes instead of a fixed idle cadence to reduce flicker while scrolling.

## [0.1.0] - 2026-02-10

### Added
- Vim-style full-page navigation with `Ctrl-f` (down) and `Ctrl-b` (up).
- Visual highlighting for search matches in the document view, with stronger emphasis on the active match.

### Changed
- Renamed the CLI and package from `mdview` to `mdvi`.
- Updated in-app title/help text and README command examples to use `mdvi`.
- Search matches are rebuilt after file reload so navigation and highlights stay accurate.
