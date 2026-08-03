use indicatif::MultiProgress;
use std::sync::atomic::Ordering::Relaxed;
use std::{
    sync::{Arc, atomic::AtomicU64},
    time::Duration,
};
use tokio::spawn;

use crate::{
    downloader::common::{CommonDownloaderError, request},
    site::instagram::Slide,
};

pub async fn fetch(
    slide: &Slide,
    m_pb: Option<MultiProgress>,
) -> Result<(), CommonDownloaderError> {
    let file_name = slide.get_file_name();

    match slide {
        Slide::Photo(p) => {
            request(&p.url, &file_name, m_pb).await?;
        }
        Slide::Video(v) => {
            let url = &v.url;
            request(url, &file_name, m_pb).await?;
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
    ///  
    /// default `thread_count` is 4
    pub async fn download(&self, jobs: Vec<Slide>, thread_count: Option<u8>) -> u64 {
        let failed_job_count = Arc::new(AtomicU64::new(0));
        let m = MultiProgress::new();

        let mut handles = vec![];

        for slide in jobs {
            let m_clone = m.clone();
            let failed_job_count_clone = Arc::clone(&failed_job_count);

            let handle = spawn(async move {
                let m = m_clone.clone();

                if let Err(err) = slide.download(Some(m_clone)).await {
                    if matches!(err, CommonDownloaderError::FileAlreadyExists(_)) {
                    } else {
                        let _ = m.println(format!(
                            "failed to download: {} -> {}",
                            slide.get_file_name(),
                            err
                        ));

                        failed_job_count_clone.fetch_add(1, Relaxed);
                    }
                }
            });
            handles.push(handle);

            if handles.len() == thread_count.unwrap_or(4) as usize {
                for handle in std::mem::take(&mut handles) {
                    let _r = handle.await.unwrap();
                }
            }
        }

        failed_job_count.load(Relaxed)
    }
}
