use anyhow::{Context, Result};
use xxer::downloader::common::CommonDownloaderError;
use xxer::site::common::{Site, WriteMetadata};
use xxer::site::instagram::{Instagram, ViewType};

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let cookie_file = &args[1];

    let slides = Instagram::new(cookie_file)
        .get(ViewType::Bookmarks, Some(20))
        .await
        .context("failed to get the ViewType")?;

    for slide in slides {
        if let Err(err) = slide.download(None).await {
            if matches!(err, CommonDownloaderError::FileAlreadyExists(_)) {
                continue;
            }

            return Err(err.into());
        }

        let file_name = slide.get_file_name();

        if let Err(err) = slide.write_metadata(&file_name) {
            eprintln!(
                "failed to write metadata for the file: {} Error: {}",
                file_name, err
            );
        }
    }

    Ok(())
}
