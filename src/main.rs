use anyhow::Result;
use clap::Parser;

mod args;
mod handlers;

use args::{Cli, Commands, XCommands};

#[tokio::main]
async fn main() -> Result<()> {
    let args = &Cli::parse();

    match &args.commands {
        Commands::X(x_command) => match x_command {
            XCommands::Bookmarks(bookmark_args) => {
                handlers::x::bookmarks(&bookmark_args, args).await?
            }
        },
    }

    Ok(())
}
