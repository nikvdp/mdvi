mod app;
mod renderer;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use directories::ProjectDirs;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ImageProtocol {
    Auto,
    Halfblocks,
    Sixel,
    Kitty,
    Iterm2,
}

#[derive(Debug, Parser)]
#[command(name = "mdvi")]
#[command(
    version,
    about = "A high-quality markdown file viewer for the terminal"
)]
struct Cli {
    /// Markdown file to open
    path: PathBuf,

    /// Start at a specific line (1-based)
    #[arg(short, long, default_value_t = 1)]
    line: usize,

    /// Read settings from a specific config file
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Image rendering protocol: auto, halfblocks, sixel, kitty, iterm2
    #[arg(long, value_enum)]
    image_protocol: Option<ImageProtocol>,

    /// Show the viewer border
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "hide_border")]
    show_border: bool,

    /// Hide the viewer border
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "show_border")]
    hide_border: bool,

    /// Show the title when borders are enabled
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "hide_title")]
    show_title: bool,

    /// Hide the title even when borders are enabled
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "show_title")]
    hide_title: bool,

    /// Show the terminal cursor during normal viewing
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "hide_cursor")]
    show_cursor: bool,

    /// Hide the terminal cursor during normal viewing
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "show_cursor")]
    hide_cursor: bool,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    image_protocol: Option<ImageProtocol>,
    show_border: Option<bool>,
    show_title: Option<bool>,
    hide_cursor: Option<bool>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let file_config = load_file_config(cli.config.as_deref())?;
    let defaults = app::ViewerOptions::default();
    let viewer_options = app::ViewerOptions {
        image_protocol: cli
            .image_protocol
            .or(file_config.image_protocol)
            .unwrap_or(defaults.image_protocol),
        show_border: resolve_toggle(cli.show_border, cli.hide_border)
            .or(file_config.show_border)
            .unwrap_or(defaults.show_border),
        show_title: resolve_toggle(cli.show_title, cli.hide_title)
            .or(file_config.show_title)
            .unwrap_or(defaults.show_title),
        hide_cursor: resolve_toggle(cli.hide_cursor, cli.show_cursor)
            .or(file_config.hide_cursor)
            .unwrap_or(defaults.hide_cursor),
    };

    app::run(cli.path, cli.line, viewer_options)
}

fn resolve_toggle(enable_flag: bool, disable_flag: bool) -> Option<bool> {
    if enable_flag {
        Some(true)
    } else if disable_flag {
        Some(false)
    } else {
        None
    }
}

fn load_file_config(config_path: Option<&Path>) -> Result<FileConfig> {
    match config_path {
        Some(path) => read_config_file(path),
        None => match default_config_path() {
            Some(path) if path.exists() => read_config_file(&path),
            _ => Ok(FileConfig::default()),
        },
    }
}

fn read_config_file(path: &Path) -> Result<FileConfig> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    toml::from_str(&contents)
        .with_context(|| format!("failed to parse config file {}", path.display()))
}

fn default_config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "mdvi").map(|dirs| dirs.config_dir().join("config.toml"))
}
