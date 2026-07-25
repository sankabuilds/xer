use colored::Colorize;
use indicatif::style::TemplateError;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use reqwest::StatusCode;
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

    print!("{}\r", &path.yellow());
    std::io::stdout().flush()?;

    let mut file = {
        match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
        {
            Ok(k) => k,
            Err(err) => {
                if err.kind() == io::ErrorKind::AlreadyExists {
                    print!("{}\n", &path.truecolor(145, 145, 145));

                    return Err(XDownloaderError::FileAlreadyExists(path.clone()));
                }

                return Err(err.into());
            }
        }
    };

    let exit_state = State::Error { path: &path };
    let mut res = reqwest::get(url).await.map_err(|err| {
        reset_terminal(&exit_state);

        err
    })?;

    if res.status() != 200 {
        reset_terminal(&exit_state);

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

    let content_length = res.content_length().unwrap_or(3e+9 as u64); // ??? idk

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

    let exit_state = State::ErrorChunk {
        pb: &pb,
        path: &path,
    };
    while let Some(chunk) = res.chunk().await.map_err(|err| {
        reset_terminal(&exit_state);

        err
    })? {
        file.write_all(&chunk)?;
        pb.inc(chunk.len() as u64);
    }

    let exit_state = State::Nominal {
        pb: &pb,
        path: &path,
    };
    reset_terminal(&exit_state);

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
