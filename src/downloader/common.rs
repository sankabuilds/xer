use colored::Colorize;
use indicatif::style::TemplateError;
use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use reqwest::header::{HeaderValue, InvalidHeaderValue, RANGE};
use reqwest::{Client, StatusCode};
use std::io::Write;
use std::{fs, io};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CommonDownloaderError {
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

    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),

    #[error("Partial request failed status code: {status_code} for ({url})")]
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

fn reset_terminal(status: &State, m_pb: &Option<MultiProgress>) {
    match status {
        State::Nominal { pb, path } => {
            pb.finish_and_clear();

            if let Some(m) = m_pb {
                let _ = m.println(format!("{}", path.green()));
            } else {
                print!("{}\n", path.green());
                std::io::stdout().flush().expect("stdout flush failed");
            }
        }
        State::Error { path } => {
            if let Some(m) = m_pb {
                let _ = m.println(format!("{}", path.red()));
            } else {
                print!("{}\n", path.red());
                std::io::stdout().flush().expect("stdout flush failed");
            }
        }
        State::ErrorChunk { pb, path } => {
            pb.finish_and_clear();

            if let Some(m) = m_pb {
                let _ = m.println(format!("{}", path.red()));
            } else {
                print!("{}\n", &path.red());
                std::io::stdout().flush().expect("stdout flush failed");
            }
        }
    }
}

pub async fn request(
    url: &str,
    file_name: &str,
    m_pb: Option<MultiProgress>,
) -> Result<(), CommonDownloaderError> {
    let path = format!("./{}", file_name);
    let partial_path = format!("{}.partial", &path);

    if m_pb.is_none() {
        print!("{}\r", &path.yellow());
        std::io::stdout().flush()?;
    }

    let mut is_partial = (false, 0_u64);
    let mut file = {
        if fs::exists(&path)? {
            if let Some(m) = m_pb {
                let _ = m.println(format!("{}", &path.truecolor(145, 145, 145)));
            } else {
                print!("{}\n", &path.truecolor(145, 145, 145));
            }

            return Err(CommonDownloaderError::FileAlreadyExists(path.clone()));
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
                    reset_terminal(&exit_state, &m_pb);
                    drop(file);
                    let _ = fs::remove_file(&partial_path);
                    return Err(err.into());
                }
            }
        } else {
            match client.get(url).send().await {
                Ok(res) => res,
                Err(err) => {
                    reset_terminal(&exit_state, &m_pb);
                    drop(file);
                    let _ = fs::remove_file(&partial_path);
                    return Err(err.into());
                }
            }
        }
    };

    if is_partial.0 {
        if res.status() != 206 {
            reset_terminal(&exit_state, &m_pb);

            return Err(CommonDownloaderError::PartialRequestFailed {
                status_code: res.status(),
                url: res.url().as_str().into(),
            });
        }
    } else {
        if res.status() != 200 {
            reset_terminal(&exit_state, &m_pb);
            drop(file);
            let _ = fs::remove_file(&partial_path);

            return Err(CommonDownloaderError::NotOk {
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

    let pb = {
        if let Some(m) = &m_pb {
            m.add(ProgressBar::new(content_length))
        } else {
            ProgressBar::new(content_length)
        }
    };
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
            reset_terminal(&exit_state, &m_pb);

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
    reset_terminal(&exit_state, &m_pb);
    drop(file);
    fs::rename(partial_path, path)?;

    Ok(())
}
