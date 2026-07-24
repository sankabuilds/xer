use std::time::Duration;

use anyhow::{Context, Result};
use xxer::{
    downloader::x::DownloaderOptions,
    site::x::{ViewType, XTwitter},
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let cookie_file = &args[1];

    let slides = XTwitter::new(cookie_file)
        .get(ViewType::Bookmarks, Some(100))
        .await
        .context("failed to get the ViewType")?;

    DownloaderOptions::new()
        .timeout(Duration::from_secs(2))
        .download(&slides)
        .await;

    Ok(())
}
