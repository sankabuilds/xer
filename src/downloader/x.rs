use std::time::Duration;
use thiserror::Error;

use crate::downloader::common::{CommonDownloaderError, request};
use crate::site::common::Quality;
use crate::site::x::Slide;

#[derive(Error, Debug)]
pub enum XDownloaderError {}

pub async fn fetch(slide: &Slide) -> Result<(), CommonDownloaderError> {
    let file_name = slide.get_file_name();

    match slide {
        Slide::Photo(p) => {
            let url = &p.media_url_https;
            request(url, &file_name).await?;
        }
        Slide::Video(v) => {
            let url = &v.video_info.get(Quality::Best).url;
            request(url, &file_name).await?;
        }
    }

    Ok(())
}

pub struct DownloaderOptions {
    timeout: Duration,
}

impl DownloaderOptions {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_millis(100),
        }
    }

    pub fn timeout(&mut self, timeout: Duration) -> &mut Self {
        self.timeout = timeout;
        self
    }

    /// returns the count of failed jobs
    pub async fn download(&self, jobs: &Vec<Slide>) -> u64 {
        let mut failed_job_count = 0_u64;

        for slide in jobs {
            if let Err(err) = slide.download().await {
                if matches!(err, CommonDownloaderError::FileAlreadyExists(_)) {
                    continue;
                }

                eprintln!("failed to download: {} -> {}", slide.get_file_name(), err);
                failed_job_count += 1;
            }

            tokio::time::sleep(self.timeout).await;
        }

        failed_job_count
    }
}
