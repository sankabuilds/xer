use clap::{Args, Parser, Subcommand};

/// 🌴 Xer for xers
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Path to the cookie file
    #[arg(short, long)]
    pub cookie: Option<String>,

    #[command(subcommand)]
    pub commands: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 𝕏 - Download X/Twitter media
    #[command(subcommand)]
    X(XCommands),
}

#[derive(Subcommand)]
pub enum XCommands {
    /// 🔖 Download bookmarks
    Bookmarks(XBookmarksArgs),
}

#[derive(Args)]
pub struct XBookmarksArgs {
    /// Download all the available bookmarks
    #[arg(short, long, default_value_t = false)]
    pub all: bool,

    /// Download bookmarks with a limit
    #[arg(short, long, default_value_t = 100)]
    pub limit: u32,
}
