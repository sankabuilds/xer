use indicatif::MultiProgress;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Duration;
use thiserror::Error;
use tokio::spawn;

use crate::downloader::common::{CommonDownloaderError, request};
use crate::site::common::{Quality, WriteMetadata};
use crate::site::x::Slide;

#[derive(Error, Debug)]
pub enum XDownloaderError {}

pub async fn fetch(
    slide: &Slide,
    m_pb: Option<MultiProgress>,
) -> Result<(), CommonDownloaderError> {
    let file_name = slide.get_file_name();

    match slide {
        Slide::Photo(p) => {
            let url = &p.media_url_https;
            request(url, &file_name, m_pb).await?;
        }
        Slide::Video(v) => {
            let url = &v.video_info.get(Quality::Best).url;
            request(url, &file_name, m_pb).await?;
        }
    }

    Ok(())
}

pub struct DownloaderOptions {
    timeout: Duration,
}

impl Default for DownloaderOptions {
    fn default() -> Self {
        Self::new()
    }
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
    ///
    /// default `thread_count` is 4
    pub async fn download(&self, jobs: Vec<Slide>, thread_count: Option<u8>) -> u64 {
        let failed_job_count = Arc::new(AtomicU64::new(0));
        let m = MultiProgress::new();
        let tc = thread_count.unwrap_or(4) as usize;
        let last_job_index = jobs.len() - 1;

        let mut handles = vec![];

        for (index, slide) in jobs.into_iter().enumerate() {
            let m_clone = m.clone();
            let failed_job_count_clone = Arc::clone(&failed_job_count);

            let handle = spawn(async move {
                let m = m_clone.clone();
                let file_name = slide.get_file_name();

                if let Err(err) = slide.download(Some(m_clone)).await {
                    if matches!(err, CommonDownloaderError::FileAlreadyExists(_)) {
                    } else {
                        let _ = m.println(format!("failed to download: {} -> {}", file_name, err));

                        failed_job_count_clone.fetch_add(1, Relaxed);
                    }
                }

                if let Err(err) = slide.write_metadata(&file_name) {
                    let _ = m.println(format!(
                        "failed to write metadata for the file: {} -> {}",
                        file_name, err
                    ));
                }
            });
            handles.push(handle);

            if handles.len() == tc || index == last_job_index {
                for handle in std::mem::take(&mut handles) {
                    if let Err(err) = handle.await {
                        eprintln!("JoinError: Task failed to execute to completion: {}", err);
                    }
                }
            }
        }

        failed_job_count.load(Relaxed)
    }
}
