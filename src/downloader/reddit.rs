use indicatif::MultiProgress;
use regex::Regex;
use std::sync::atomic::Ordering::Relaxed;
use std::{
    sync::{Arc, atomic::AtomicU64},
    time::Duration,
};
use thiserror::Error;
use tokio::spawn;

use crate::downloader::common::request_hls;
use crate::site::common::WriteMetadata;
use crate::site::reddit::PostDomain;
use crate::{
    downloader::common::{CommonDownloaderError, request},
    site::reddit::Slide,
};

#[derive(Error, Debug)]
pub enum RedditDownloaderError {
    #[error("Redgif most likey deleted: {0}")]
    VideoDeleted(String),

    #[error("CommonDownloaderError: {0}")]
    CommonDownloaderError(#[from] CommonDownloaderError),
}

async fn get_redgifs_url(post_url: &str) -> Result<String, RedditDownloaderError> {
    let res = reqwest::get(post_url).await.unwrap();

    let body = res.text().await.unwrap();
    let re = Regex::new(r#"contentUrl":"(.+)", *"creator"#).unwrap();

    let caps = re
        .captures(&body)
        .ok_or_else(|| RedditDownloaderError::VideoDeleted(post_url.into()))?;

    Ok(caps.get(1).unwrap().as_str().replace("-silent", ""))
}

pub async fn fetch(
    slide: &Slide,
    m_pb: Option<MultiProgress>,
) -> Result<(), RedditDownloaderError> {
    let file_name = slide.get_file_name();

    match slide {
        Slide::Photo(p) => {
            request(&p.url, &file_name, m_pb).await?;
        }
        Slide::Video(v) => {
            if matches!(v.site, PostDomain::Redgifs) {
                let redgif_url = get_redgifs_url(&v.url).await?;
                request(&redgif_url, &file_name, m_pb).await?;
            } else if matches!(v.site, PostDomain::VReddIt) {
                request_hls(&v.url, &file_name, m_pb).await?;
            } else {
                let url = &v.url;
                request(url, &file_name, m_pb).await?;
            }
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

                if let Err(err) = slide.download(Some(m_clone)).await {
                    if matches!(
                        err,
                        RedditDownloaderError::CommonDownloaderError(
                            CommonDownloaderError::FileAlreadyExists(_)
                        )
                    ) {
                        return;
                    } else {
                        let _ = m.println(format!(
                            "failed to download: {} -> {}",
                            slide.get_file_name(),
                            err
                        ));

                        failed_job_count_clone.fetch_add(1, Relaxed);
                        return;
                    }
                }

                let file_name = slide.get_file_name();

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
