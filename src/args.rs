use clap::{Args, Parser, Subcommand};

/// 🌴 Xer for xers
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Path to the cookie file
    #[arg(short, long)]
    pub cookie: Option<String>,

    /// Verbose mode
    #[arg(short, long)]
    pub verbose: bool,

    #[command(subcommand)]
    pub commands: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 𝕏 - Download X/Twitter media
    #[command(subcommand)]
    X(XCommands),

    /// Instagram - Download Instagram media
    #[command(subcommand)]
    Gram(InstagramCommands),
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

    #[arg(long, hide = true, default_value_t = 100)]
    pub timeout: u64,

    #[arg(long, hide = true, default_value_t = 4)]
    pub thread_count: u8,
}

#[derive(Subcommand)]
pub enum InstagramCommands {
    /// 🔖 Download bookmarks
    Bookmarks(InstagramBookmarksArgs),
}

#[derive(Args)]
pub struct InstagramBookmarksArgs {
    /// Download all the available bookmarks
    #[arg(short, long, default_value_t = false)]
    pub all: bool,

    /// Download bookmarks with a limit
    #[arg(short, long, default_value_t = 100)]
    pub limit: u32,

    #[arg(long, hide = true, default_value_t = 100)]
    pub timeout: u64,

    #[arg(long, hide = true, default_value_t = 4)]
    pub thread_count: u8,
}
