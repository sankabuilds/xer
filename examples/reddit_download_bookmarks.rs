use std::str::FromStr;

use anyhow::{Context, Result};
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;
use xxer::{
    downloader::common::CommonDownloaderError,
    site::{
        common::{Site, WriteMetadata},
        reddit::{Reddit, ViewType},
    },
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let cookie_file = &args[1];

    if args.len() >= 3 {
        let debug = &args[2];
        if !debug.is_empty() {
            tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::from_str("xxer=trace,reddit_download_bookmarks=trace").unwrap(),
                )
                .init();
        }
    }

    info!("Getting slides");
    let slides = Reddit::new(cookie_file)
        .get(ViewType::Bookmarks, Some(100))
        .await
        .context("failed to get the ViewType")?;

    for slide in &slides {
        debug!(slide = ?slide, "Downloading");
        if let Err(err) = slide.download(None).await {
            if matches!(err, CommonDownloaderError::FileAlreadyExists(_)) {
                debug!(slide = ?slide, "Already exists. Skipping.");

                continue;
            }

            return Err(err.into());
        }

        let filename = slide.get_file_name();

        if let Err(err) = slide.write_metadata(filename) {
            eprintln!("{err}");
        }
    }

    Ok(())
}
