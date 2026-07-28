use anyhow::{Context, Result};
use xxer::downloader::common::CommonDownloaderError;
use xxer::site::common::Site;
use xxer::site::instagram::{Instagram, ViewType};

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let cookie_file = &args[1];

    let slides = Instagram::new(cookie_file)
        .get(ViewType::Bookmarks, None)
        .await
        .context("faild to get the ViewType")?;

    for slide in slides {
        if let Err(err) = slide.download().await {
            if matches!(err, CommonDownloaderError::FileAlreadyExists(_)) {
                continue;
            }

            return Err(err.into());
        }
    }

    Ok(())
}
