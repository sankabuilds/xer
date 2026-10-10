use anyhow::Result;
use clap::Parser;

mod args;
mod handlers;

use args::{Cli, Commands, InstagramCommands, XCommands};

use crate::args::RedditCommands;

#[tokio::main]
async fn main() -> Result<()> {
    let args = &Cli::parse();

    if args.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("xer=info,xxer=info")
            .init();
    } else if args.debug {
        tracing_subscriber::fmt()
            .with_env_filter("xer=trace,xxer=trace")
            .init();
    }

    match &args.commands {
        Commands::X(x_command) => match x_command {
            XCommands::Bookmarks(bookmark_args) => {
                handlers::x::bookmarks(bookmark_args, args).await?
            }
        },
        Commands::Gram(gram_command) => match gram_command {
            InstagramCommands::Bookmarks(bookmarks_args) => {
                handlers::instagram::bookmarks(bookmarks_args, args).await?
            }
        },
        Commands::Redd(redd_command) => match redd_command {
            RedditCommands::Bookmarks(bookmarks_args) => {
                handlers::reddit::bookmarks(bookmarks_args, args).await?
            }
        },
    }

    Ok(())
}
