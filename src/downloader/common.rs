use colored::Colorize;
use ffmpeg_cli::{FfmpegBuilder, File, Parameter};
use indicatif::style::TemplateError;
use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use m3u8_rs::{VariantStream, parse_master_playlist};
use reqwest::header::{HeaderValue, InvalidHeaderValue, RANGE};
use reqwest::{Client, StatusCode};
use std::io::Write;
use std::process::Stdio;
use std::{fs, io};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info, instrument};

use crate::downloader::utils::get_progress_bar;

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

    #[error("Failed to download the slide. A file with the same name already exists: {0}")]
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

    #[error("FfmpegCli Error: {0}")]
    FfmpegCliError(String),

    #[error("m3u8_rs Error: {0}")]
    M3u8RsError(String),
}

enum State<'a, 'b> {
    Nominal { pb: &'a ProgressBar, path: &'b str },
    Error { path: &'b str },
    ErrorChunk { pb: &'a ProgressBar, path: &'b str },
}

fn reset_terminal(status: &State, m_pb: &Option<MultiProgress>) -> std::io::Result<()> {
    match status {
        State::Nominal { pb, path } => {
            pb.finish_and_clear();

            if let Some(m) = m_pb {
                m.println(format!("{}", path.green()))?;
            } else {
                println!("{}", path.green());
                std::io::stdout().flush().expect("stdout flush failed");
            }
        }
        State::Error { path } => {
            if let Some(m) = m_pb {
                m.println(format!("{}", path.red()))?;
            } else {
                println!("{}", path.red());
                std::io::stdout().flush().expect("stdout flush failed");
            }
        }
        State::ErrorChunk { pb, path } => {
            pb.finish_and_clear();

            if let Some(m) = m_pb {
                m.println(format!("{}", path.red()))?;
            } else {
                println!("{}", path.red());
                std::io::stdout().flush().expect("stdout flush failed");
            }
        }
    }

    Ok(())
}

pub async fn request(
    url: &str,
    file_name: &str,
    m_pb: Option<MultiProgress>,
) -> Result<(), CommonDownloaderError> {
    let path = format!("./{}", file_name);
    let partial_path = format!("{}.partial", path);

    if m_pb.is_none() {
        print!("{}\r", path.yellow());
        std::io::stdout().flush()?;
    }

    let mut is_partial = (false, 0_u64);
    let mut file = {
        if fs::exists(&path)? {
            if let Some(m) = m_pb {
                if let Err(err) = m.println(format!("{}", path.truecolor(145, 145, 145))) {
                    eprintln!("warning: failed to reset terminal: {}", err);
                }
            } else {
                println!("{}", path.truecolor(145, 145, 145));
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
                    fs::OpenOptions::new().append(true).open(&partial_path)?
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
                    if let Err(err) = reset_terminal(&exit_state, &m_pb) {
                        eprintln!("warning: failed to reset terminal: {}", err);
                    }
                    drop(file);
                    let _ = fs::remove_file(&partial_path);
                    return Err(err.into());
                }
            }
        } else {
            match client.get(url).send().await {
                Ok(res) => res,
                Err(err) => {
                    if let Err(err) = reset_terminal(&exit_state, &m_pb) {
                        eprintln!("warning: failed to reset terminal: {}", err);
                    }
                    drop(file);
                    let _ = fs::remove_file(&partial_path);
                    return Err(err.into());
                }
            }
        }
    };

    if is_partial.0 {
        if res.status() != 206 {
            if let Err(err) = reset_terminal(&exit_state, &m_pb) {
                eprintln!("warning: failed to reset terminal: {}", err);
            }

            return Err(CommonDownloaderError::PartialRequestFailed {
                status_code: res.status(),
                url: res.url().as_str().into(),
            });
        }
    } else {
        if res.status() != 200 {
            if let Err(err) = reset_terminal(&exit_state, &m_pb) {
                eprintln!("warning: failed to reset terminal: {}", err);
            }
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

    pb.set_prefix(format!("{}", file_name.replace(".partial", "").yellow()));
    if is_partial.0 {
        pb.set_position(is_partial.1);
    }

    let exit_state = State::ErrorChunk {
        pb: &pb,
        path: &path,
    };
    while let Some(chunk) = match res.chunk().await {
        Err(err) => {
            if let Err(err) = reset_terminal(&exit_state, &m_pb) {
                eprintln!("warning: failed to reset terminal: {}", err);
            }

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
    if let Err(err) = reset_terminal(&exit_state, &m_pb) {
        eprintln!("warning: failed to reset terminal: {}", err);
    }
    drop(file);
    fs::rename(partial_path, path)?;

    Ok(())
}

#[instrument]
async fn fetch_file(
    file_url: &str,
    pb_prefix: &str,
    file: &mut tokio::fs::File,
    partial_length: Option<u64>,
) -> Result<(), CommonDownloaderError> {
    info!("Fetching file");

    let client = reqwest::Client::new();

    let mut res = {
        if let Some(length) = partial_length {
            info!("Partially downloaded file.");

            debug!("Sending a range request");
            let res = client
                .get(file_url)
                .header(
                    RANGE,
                    HeaderValue::from_str(&format!("bytes={}-", length)).unwrap(),
                )
                .send()
                .await?;

            let status = res.status().as_u16();
            if status != StatusCode::PARTIAL_CONTENT {
                return Err(CommonDownloaderError::PartialRequestFailed {
                    status_code: res.status(),
                    url: res.url().as_str().into(),
                });
            }
            debug!(status = status, "Range request was successful");

            res
        } else {
            reqwest::get(file_url).await?
        }
    };

    let content_len = res.content_length().unwrap_or(3e+9 as u64) + partial_length.unwrap_or(0);

    let pb = get_progress_bar(content_len, pb_prefix);
    if let Some(length) = partial_length {
        pb.inc(length);
    }

    while let Some(chuck) = res.chunk().await? {
        file.write_all(&chuck).await?;
        file.flush().await?;

        pb.inc(chuck.len() as u64);
    }

    pb.finish_and_clear();

    Ok(())
}

async fn get_partial_file(
    partial_file_path: &str,
    partial_length: &mut Option<u64>,
) -> Result<tokio::fs::File, CommonDownloaderError> {
    let file = {
        let mut op = tokio::fs::OpenOptions::new();

        match op
            .create_new(true)
            .write(true)
            .open(partial_file_path)
            .await
        {
            Err(err) => {
                if err.kind() == io::ErrorKind::AlreadyExists {
                    // we can continue downloading the rest of the file
                    let file_meta = fs::metadata(partial_file_path)?;

                    *partial_length = Some(file_meta.len());
                    tokio::fs::OpenOptions::new()
                        .append(true)
                        .open(partial_file_path)
                        .await?
                } else {
                    return Err(err.into());
                }
            }
            Ok(f) => f,
        }
    };

    Ok(file)
}

struct AudioVideo {
    video_url: String,
    audio_url: Option<String>,
}

#[instrument]
async fn get_audio_video_from_master_playlist(
    mp_url: &str,
) -> Result<AudioVideo, CommonDownloaderError> {
    let master_playlist = reqwest::get(mp_url).await?.text().await?;
    debug!(master_playlist_response = master_playlist);

    info!("Parsing Master Playlist");
    let (_, mut mp) = parse_master_playlist(master_playlist.as_bytes()).map_err(|err| {
        error!(%err, "Master Playlist parsing failed");
        CommonDownloaderError::M3u8RsError(format!("Master Playlist parsing failed: {err}"))
    })?;

    mp.variants.sort_by_key(|v| v.bandwidth);

    let audio_streams = mp.alternatives;
    debug!(audio_streams = ?audio_streams);

    let VariantStream {
        uri: best_video_media_playlist_uri,
        audio: audio_id,
        ..
    } = mp.variants.last().unwrap();

    if let Some(id) = audio_id {
        let best_audio_media_playlist_uri = audio_streams
            .iter()
            .find(|v| &v.group_id == id)
            .unwrap()
            .uri
            .as_ref()
            .unwrap();

        let (audio_url, video_url) = {
            if best_video_media_playlist_uri.contains("CMAF") {
                let audio_url = mp_url
                    .replace("HLSPlaylist.m3u8", best_audio_media_playlist_uri)
                    .replace("m3u8", "mp4");
                let video_url = mp_url
                    .replace("HLSPlaylist.m3u8", best_video_media_playlist_uri)
                    .replace("m3u8", "mp4");

                (audio_url, video_url)
            } else {
                let audio_url = mp_url
                    .replace("HLSPlaylist.m3u8", best_audio_media_playlist_uri)
                    .replace("m3u8", "aac");
                let video_url = mp_url
                    .replace("HLSPlaylist.m3u8", best_video_media_playlist_uri)
                    .replace("m3u8", "ts");

                (audio_url, video_url)
            }
        };

        Ok(AudioVideo {
            audio_url: Some(audio_url),
            video_url,
        })
    } else {
        let video_url = {
            if best_video_media_playlist_uri.contains("CMAF") {
                mp_url
                    .replace("HLSPlaylist.m3u8", best_video_media_playlist_uri)
                    .replace("m3u8", "mp4")
            } else {
                mp_url
                    .replace("HLSPlaylist.m3u8", best_video_media_playlist_uri)
                    .replace("m3u8", "ts")
            }
        };

        Ok(AudioVideo {
            audio_url: None,
            video_url,
        })
    }
}

/// Written specifically for reddit. May need adjustments to make this more generic
#[instrument]
pub async fn request_hls(
    playlist_url: &str,
    file_name: &str,
    _m_pb: Option<MultiProgress>,
) -> Result<(), CommonDownloaderError> {
    let path = format!("./{}", file_name);

    let video_partial_path = format!("{}-video.partial", path);
    let audio_partial_path = format!("{}-audio.partial", path);

    let video_path = video_partial_path.replace(".partial", "");
    let audio_path = audio_partial_path.replace(".partial", "");

    if fs::exists(&path)? {
        // TODO
        // is the file incomplete? if so, we should remove it
        return Err(CommonDownloaderError::FileAlreadyExists(path));
    }

    let AudioVideo {
        audio_url,
        video_url,
    } = get_audio_video_from_master_playlist(playlist_url).await?;

    if let Some(audio_url) = audio_url {
        info!("Video URL" = video_url, "Audio URL" = audio_url);

        // VIDEO
        if !tokio::fs::try_exists(&video_path).await? {
            let mut partial_length = None;
            let mut partial_video_file =
                get_partial_file(&video_partial_path, &mut partial_length).await?;

            fetch_file(
                &video_url,
                &video_partial_path,
                &mut partial_video_file,
                partial_length,
            )
            .await?;
            tokio::fs::rename(&video_partial_path, &video_path).await?;
        } else {
            debug!(video_path = video_path, "Video already exists");
        }

        // AUDIO
        if !tokio::fs::try_exists(&audio_path).await? {
            let mut partial_length = None;
            let mut partial_audio_file =
                get_partial_file(&audio_partial_path, &mut partial_length).await?;

            fetch_file(
                &audio_url,
                &audio_partial_path,
                &mut partial_audio_file,
                partial_length,
            )
            .await?;
            tokio::fs::rename(&audio_partial_path, &audio_path).await?;
        } else {
            debug!(audio_path = audio_path, "Audio already exists");
        }

        // muxxing
        info!(Audio = audio_path, Video = video_path, "Muxing");
        muxx(Some(&audio_path), &video_path, &path).await?;
        debug!(
            audio_path = audio_path,
            video_path = video_path,
            "Removing temporary audio & video files"
        );
        tokio::fs::remove_file(audio_path).await?;
        tokio::fs::remove_file(video_path).await?;

        println!("{}", path.green());
    } else {
        info!("Video Only file");

        if !tokio::fs::try_exists(&video_path).await? {
            let mut partial_length = None;
            let mut partial_video_file =
                get_partial_file(&video_partial_path, &mut partial_length).await?;

            fetch_file(
                &video_url,
                &video_partial_path,
                &mut partial_video_file,
                partial_length,
            )
            .await?;
            tokio::fs::rename(&video_partial_path, &video_path).await?;
        } else {
            debug!(video_path = video_path, "Video already exists");
        }

        debug!(video_path = video_path, "Muxing a video only file"); //wait, what?
        muxx(None, &video_path, &path).await?;
        debug!(video_path = video_path, "Removing temporary video file");
        tokio::fs::remove_file(video_path).await?;

        println!("{}", path.green());
    }

    Ok(())
}

#[instrument]
async fn muxx(
    a_path: Option<&str>,
    v_path: &str,
    final_path: &str,
) -> Result<(), CommonDownloaderError> {
    if let Some(a_path) = a_path {
        let ffmpeg = FfmpegBuilder::new()
            .stderr(Stdio::piped())
            .option(Parameter::Single("y"))
            .input(File::new(v_path))
            .input(File::new(a_path))
            .output(File::new(final_path).option(Parameter::KeyValue("c", "copy")))
            .run()
            .await
            .map_err(|err| CommonDownloaderError::FfmpegCliError(err.to_string()))?;

        let output = ffmpeg.process.wait_with_output()?;

        if !output.status.success() {
            let _stderr = String::from_utf8_lossy(&output.stderr);
            // TODO
            panic!("FFMPEG command failed");
        }
    } else {
        debug!("Video only");

        let ffmpeg = FfmpegBuilder::new()
            .stderr(Stdio::piped())
            .option(Parameter::Single("y"))
            .input(File::new(v_path))
            .output(File::new(final_path).option(Parameter::KeyValue("c", "copy")))
            .run()
            .await
            .map_err(|err| CommonDownloaderError::FfmpegCliError(err.to_string()))?;

        let output = ffmpeg.process.wait_with_output()?;

        if !output.status.success() {
            let _stderr = String::from_utf8_lossy(&output.stderr);
            // TODO
            panic!("FFMPEG command failed");
        }
    }

    Ok(())
}
