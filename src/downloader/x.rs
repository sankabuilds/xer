use colored::Colorize;
use indicatif::style::TemplateError;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use reqwest::header::{HeaderValue, InvalidHeaderValue, RANGE};
use reqwest::{Client, StatusCode};
use std::io::Write;
use std::time::Duration;
use std::{fs, io};
use thiserror::Error;

use crate::site::x::{Quality, Slide};

#[derive(Error, Debug)]
pub enum XDownloaderError {
    #[error("file I/O failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP request failed: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error(
        "HTTP request failed. ({url}) Status Code: {status_code}. Response Body: {response_body}"
    )]
    NotOk {
        url: String,
        status_code: StatusCode,
        response_body: String,
    },

    #[error("Failed to download the slide. A file with same name already exists: {0}")]
    FileAlreadyExists(String),

    #[error("failed to set up the progress bar: {0}")]
    Indicatif(#[from] TemplateError),

    #[error("failed to set up the progress bar: {0}")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),

    #[error("partial request failed status code: {status_code} for ({url})")]
    PartialRequestFailed {
        status_code: StatusCode,
        url: String,
    },
}

enum State<'a, 'b> {
    Nominal { pb: &'a ProgressBar, path: &'b str },
    Error { path: &'b str },
    ErrorChunk { pb: &'a ProgressBar, path: &'b str },
}

fn reset_terminal(status: &State) {
    match status {
        State::Nominal { pb, path } => {
            pb.finish_and_clear();
            print!("{}\n", path.green());
            std::io::stdout().flush().expect("stdout flush failed");
        }
        State::Error { path } => {
            print!("{}\n", path.red());
            std::io::stdout().flush().expect("stdout flush failed");
        }
        State::ErrorChunk { pb, path } => {
            pb.finish_and_clear();
            print!("{}\n", &path.red());
            std::io::stdout().flush().expect("stdout flush failed");
        }
    }
}

async fn request(url: &str, file_name: &str) -> Result<(), XDownloaderError> {
    let path = format!("./{}", file_name);
    let partial_path = format!("{}.partial", &path);

    print!("{}\r", &path.yellow());
    std::io::stdout().flush()?;

    let mut is_partial = (false, 0_u64);
    let mut file = {
        if fs::exists(&path)? {
            print!("{}\n", &path.truecolor(145, 145, 145));

            return Err(XDownloaderError::FileAlreadyExists(path.clone()));
        }

        match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&partial_path)
        {
            Ok(k) => k,
            Err(err) => {
                if err.kind() == io::ErrorKind::AlreadyExists {
                    // we can continue downloading the rest of the file
                    let file_meta = fs::metadata(&partial_path)?;

                    is_partial = (true, file_meta.len());
                    fs::OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(&partial_path)?
                } else {
                    return Err(err.into());
                }
            }
        }
    };

    let client = Client::new();
    let exit_state = State::Error { path: &path };
    let mut res = {
        if is_partial.0 {
            match client
                .get(url)
                .header(
                    RANGE,
                    HeaderValue::from_str(&format!("bytes={}-", is_partial.1))?,
                )
                .send()
                .await
            {
                Ok(res) => res,
                Err(err) => {
                    reset_terminal(&exit_state);
                    drop(file);
                    let _ = fs::remove_file(&partial_path);
                    return Err(err.into());
                }
            }
        } else {
            match client.get(url).send().await {
                Ok(res) => res,
                Err(err) => {
                    reset_terminal(&exit_state);
                    drop(file);
                    let _ = fs::remove_file(&partial_path);
                    return Err(err.into());
                }
            }
        }
    };

    if is_partial.0 {
        if res.status() != 206 {
            reset_terminal(&exit_state);

            return Err(XDownloaderError::PartialRequestFailed {
                status_code: res.status(),
                url: res.url().as_str().into(),
            });
        }
    } else {
        if res.status() != 200 {
            reset_terminal(&exit_state);
            drop(file);
            let _ = fs::remove_file(&partial_path);

            return Err(XDownloaderError::NotOk {
                status_code: res.status(),
                url: res.url().as_str().into(),
                response_body: {
                    let mut body = res.text().await.unwrap_or(
                        "Couldn't get the response body. Error while fetching the body".into(),
                    );

                    if body.chars().count() == 0 {
                        body = "Empty".into()
                    }

                    body
                },
            });
        }
    }

    let content_length = {
        if is_partial.0 {
            res.content_length().unwrap_or(3e+9 as u64) + is_partial.1
        } else {
            res.content_length().unwrap_or(3e+9 as u64)
        }
    }; // ??? idk

    let pb = ProgressBar::new(content_length);
    pb.set_style(
        ProgressStyle::with_template(
            "{prefix} [{elapsed_precise}] [{wide_bar:.green/yellow}] {bytes}/{total_bytes} ({eta})",
        )?
        .with_key(
            "eta",
            |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                let _ = write!(w, "{:.1}s", state.eta().as_secs_f64());
            },
        )
        .progress_chars("#>-"),
    );

    pb.set_prefix(format!("{}", file_name.yellow()));
    if is_partial.0 {
        pb.set_position(is_partial.1);
    }

    let exit_state = State::ErrorChunk {
        pb: &pb,
        path: &path,
    };
    while let Some(chunk) = match res.chunk().await {
        Err(err) => {
            reset_terminal(&exit_state);

            return Err(err.into());
        }
        Ok(k) => k,
    } {
        file.write_all(&chunk)?;
        pb.inc(chunk.len() as u64);
    }

    let exit_state = State::Nominal {
        pb: &pb,
        path: &path,
    };
    reset_terminal(&exit_state);
    drop(file);
    fs::rename(partial_path, path)?;

    Ok(())
}

pub async fn fetch(slide: &Slide) -> Result<(), XDownloaderError> {
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
                if matches!(err, XDownloaderError::FileAlreadyExists(_)) {
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
