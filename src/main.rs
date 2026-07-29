use anyhow::Result;
use clap::Parser;

mod args;
mod handlers;

use args::{Cli, Commands, InstagramCommands, XCommands};

#[tokio::main]
async fn main() -> Result<()> {
    let args = &Cli::parse();

    if args.verbose {
        env_logger::builder()
            .filter_level(log::LevelFilter::Info)
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
    }

    Ok(())
}
