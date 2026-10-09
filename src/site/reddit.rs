#![allow(dead_code)]

use chrono::{DateTime, FixedOffset};
use indicatif::MultiProgress;
use reqwest::{StatusCode, Url};
use scraper::{ElementRef, Html, Selector};
use std::{fmt::Display, sync::Arc};
use thiserror::Error;
use tracing::{debug, info};

use crate::{
    cookie::reddit::{get_jar, new_loaded_client},
    downloader::{self, common::CommonDownloaderError},
    site::common::{
        self, Site, VideoMetadataTag, WriteMetadata, w_photo_metadata, w_video_metadata,
    },
};

pub const REDDIT: &str = "https://www.reddit.com";

#[derive(Error, Debug)]
pub enum RedditError {
    #[error("Io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("Unexpected response: expected `{expect}` from the document: {document}")]
    UnexpectedDocumentStructure { expect: String, document: String },

    #[error("No bookmarks available in your Instagram account")]
    ZeroBookmarks,

    #[error("Chrono parse error: {0}")]
    ChronoParseError(#[from] chrono::ParseError),
}

#[derive(Debug)]
pub enum PostDomain {
    Reddit,
    Redgifs,
    IRedgifs,
    IReddIt,
    VReddIt,
    Unknown(String),
}

impl From<&str> for PostDomain {
    fn from(domain: &str) -> Self {
        match domain {
            "redgifs.com" | "v3.redgifs.com" => PostDomain::Redgifs,
            "i.redd.it" => PostDomain::IReddIt,
            "v.redd.it" => PostDomain::VReddIt,
            "reddit.com" => PostDomain::Reddit,
            "i.redgifs.com" => PostDomain::IRedgifs,
            other => PostDomain::Unknown(other.into()),
        }
    }
}

#[derive(Debug)]
pub struct Photo {
    pub url: String,
    pub site: PostDomain,
    pub created_timestamp: DateTime<FixedOffset>,
    pub author: String,
    pub author_id: String,
    pub permalink: String,
}

impl Display for Photo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.url)
    }
}

impl Photo {
    fn get_file_name(&self) -> String {
        match self.site {
            PostDomain::IReddIt | PostDomain::Reddit => {
                let url = self.url.parse::<Url>().unwrap();
                let mut split = url.path().split(".");

                let filename = split.next().map(|f| &f[1..]);
                let ext = split.last();

                if let Some(ext) = ext
                    && let Some(filename) = filename
                {
                    self.created_timestamp.timestamp().to_string() + "_" + filename + "." + ext
                } else {
                    self.created_timestamp.timestamp().to_string()
                }
            }
            PostDomain::Redgifs => self.created_timestamp.timestamp().to_string() + ".jpg",
            PostDomain::IRedgifs => self.created_timestamp.timestamp().to_string() + ".jpg",
            PostDomain::VReddIt => self.created_timestamp.timestamp().to_string() + ".jpg",
            PostDomain::Unknown(_) => {
                unimplemented!()
            }
        }
    }
}

#[derive(Debug)]
pub struct Video {
    pub url: String,
    pub site: PostDomain,
    pub created_timestamp: DateTime<FixedOffset>,
    pub author: String,
    pub author_id: String,
    pub permalink: String,
}

impl Display for Video {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.url)
    }
}

impl Video {
    fn get_file_name(&self) -> String {
        match self.site {
            PostDomain::IReddIt | PostDomain::Reddit => {
                let url = self.url.parse::<Url>().unwrap();
                let mut split = url.path().split(".");

                let filename = split.next().map(|f| &f[1..]);
                let ext = split.last();

                if let Some(ext) = ext
                    && let Some(filename) = filename
                {
                    self.created_timestamp.timestamp().to_string() + "_" + filename + "." + ext
                } else {
                    self.created_timestamp.timestamp().to_string()
                }
            }
            PostDomain::Redgifs => self.created_timestamp.timestamp().to_string() + ".mp4",
            PostDomain::IRedgifs => self.created_timestamp.timestamp().to_string() + ".mp4",
            PostDomain::VReddIt => self.created_timestamp.timestamp().to_string() + ".mp4",
            PostDomain::Unknown(_) => {
                unimplemented!()
            }
        }
    }
}

pub type Slide = common::Slide<Photo, Video>;

impl WriteMetadata for Slide {
    fn write_metadata<P: AsRef<std::path::Path>>(
        &self,
        file_path: P,
    ) -> Result<(), common::MetadataError> {
        match self {
            Self::Photo(p) => {
                let author = format!("{} ({})", p.author, p.author_id);

                w_photo_metadata(
                    file_path.as_ref(),
                    common::ImageDescription {
                        author: &author,
                        post_url: &p.permalink,
                        tags: None,
                    },
                )
            }
            Self::Video(v) => {
                let author = format!("{} ({})", v.author, v.author_id);

                let tags = vec![
                    VideoMetadataTag::Author(&author),
                    VideoMetadataTag::PostUrl(&v.permalink),
                ];

                w_video_metadata(file_path.as_ref(), tags)
            }
        }
    }
}

impl Display for Slide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Photo(p) => write!(f, "{}", p),

            Self::Video(v) => write!(f, "{}", v),
        }
    }
}

impl Slide {
    pub fn get_file_name(&self) -> String {
        match self {
            Self::Photo(p) => p.get_file_name(),
            Self::Video(v) => v.get_file_name(),
        }
    }

    pub async fn download(&self, m_pb: Option<MultiProgress>) -> Result<(), CommonDownloaderError> {
        downloader::reddit::fetch(self, m_pb).await
    }
}

pub enum ViewType {
    Bookmarks,
}

#[derive(Debug)]
pub struct Reddit {
    pub client: reqwest::Client,
}

enum PostType {
    Gallery,
    Crosspost,
    Link,
    Video,
}

impl Site for Reddit {
    type Error = RedditError;
    type Slide = Slide;
    type ViewType = ViewType;

    fn new(cookie_file: &str) -> Self {
        let jar = Arc::new(get_jar(cookie_file));

        Self {
            client: new_loaded_client(Arc::clone(&jar)),
        }
    }

    async fn get(&self, t: ViewType, limit: Option<u32>) -> Result<Vec<Slide>, RedditError> {
        match t {
            ViewType::Bookmarks => self.get_bookmarks(limit).await,
        }
    }
}

impl Reddit {
    async fn get_bookmarks(&self, limit: Option<u32>) -> Result<Vec<Slide>, RedditError> {
        let mut slides = Vec::new();
        let mut url: Option<String> = Some(format!("{}/user/me/saved/", REDDIT));

        while let Some(d_url) = url {
            if let Some(limit) = limit
                && slides.len() > limit as usize
            {
                info!(limit = limit, "Stoping navigation. Limit reached.");
                break;
            }

            info!(url = d_url, "Requesting page");

            let req = self.client.get(d_url).header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36");

            let res = req.send().await?;

            if res.status() != StatusCode::OK {
                panic!("request failed! status: {}", res.status());
            }

            let body = res.text().await?;

            let posts_selector = Selector::parse("shreddit-post").unwrap();

            let doc = Html::parse_document(&body);

            let mut posts = Vec::new();

            for child in doc.select(&posts_selector) {
                posts.push(child);
            }

            info!("Parsing slides");
            parse_slides(&posts, &mut slides)?;
            debug!(slide_count = slides.len());

            info!("Parsing next navigation URL");
            if let Some(n_url) = get_next_navigation_url(&doc) {
                debug!(next_navigation_url = n_url);
                url = Some(format!("{}{}", REDDIT, n_url));
            } else {
                info!("Navigation ended");
                url = None;
            }
        }

        Ok(slides)
    }
}

fn parse_slides(posts: &Vec<ElementRef<'_>>, slides: &mut Vec<Slide>) -> Result<(), RedditError> {
    let img_selector = Selector::parse("zoomable-img > img").unwrap();

    for p in posts {
        let Some(domain) = p.attr("domain").map(PostDomain::from) else {
            continue;
        };
        let author = p
            .attr("author")
            .ok_or_else(|| RedditError::UnexpectedDocumentStructure {
                expect: "author".to_string(),
                document: p.html(),
            })?;

        let author_id = {
            if author == "[deleted]" {
                "Deleted"
            } else {
                p.attr("author-id")
                    .ok_or_else(|| RedditError::UnexpectedDocumentStructure {
                        expect: "author-id".to_string(),
                        document: p.html(),
                    })?
            }
        };

        let permalink =
            p.attr("permalink")
                .ok_or_else(|| RedditError::UnexpectedDocumentStructure {
                    expect: "permalink".to_string(),
                    document: p.html(),
                })?;

        let timestamp = p.attr("created-timestamp").ok_or_else(|| {
            RedditError::UnexpectedDocumentStructure {
                expect: "created-timestamp".to_string(),
                document: p.html(),
            }
        })?;
        let format = "%Y-%m-%dT%H:%M:%S%.6f%z";

        match domain {
            PostDomain::Redgifs => {
                if let Some(url) = p.attr("content-href") {
                    let slide = Slide::Video(Video {
                        url: url.to_owned(),
                        site: PostDomain::Redgifs,
                        created_timestamp: DateTime::parse_from_str(timestamp, format)?,
                        author: author.to_owned(),
                        author_id: author_id.to_owned(),
                        permalink: permalink.to_owned(),
                    });

                    slides.push(slide);
                }
            }
            PostDomain::IRedgifs => {
                let el = p.select(&img_selector).last().unwrap();

                if let Some(url) = el.attr("src") {
                    let slide = Slide::Photo(Photo {
                        url: url.to_owned(),
                        site: PostDomain::IRedgifs,
                        created_timestamp: DateTime::parse_from_str(timestamp, format)?,
                        author: author.to_owned(),
                        author_id: author_id.to_owned(),
                        permalink: permalink.to_owned(),
                    });

                    slides.push(slide);
                }
            }
            PostDomain::IReddIt => {
                let post_type = p.attr("post-type").unwrap();

                if post_type == "gif" {
                    if let Some(url) = p.attr("content-href") {
                        let slide = Slide::Photo(Photo {
                            url: url.to_owned(),
                            site: PostDomain::IReddIt,
                            created_timestamp: DateTime::parse_from_str(timestamp, format)?,
                            author: author.to_owned(),
                            author_id: author_id.to_owned(),
                            permalink: permalink.to_owned(),
                        });

                        slides.push(slide);
                    }
                } else {
                    let el = p
                        .select(&img_selector)
                        .last()
                        .ok_or_else(|| p.html())
                        .unwrap();

                    if let Some(url) = el.attr("src") {
                        let slide = Slide::Photo(Photo {
                            url: url.to_owned(),
                            site: PostDomain::IReddIt,
                            created_timestamp: DateTime::parse_from_str(timestamp, format)?,
                            author: author.to_owned(),
                            author_id: author_id.to_owned(),
                            permalink: permalink.to_owned(),
                        });

                        slides.push(slide);
                    }
                }
            }
            PostDomain::VReddIt => {
                let source_selector = Selector::parse("shreddit-player > source").unwrap();

                let el = p.select(&source_selector).last().unwrap();

                if let Some(s) = el.attr("src") {
                    let slide = Slide::Video(Video {
                        url: s.into(),
                        site: PostDomain::VReddIt,
                        created_timestamp: DateTime::parse_from_str(timestamp, format)?,
                        author: author.to_owned(),
                        author_id: author_id.to_owned(),
                        permalink: permalink.to_owned(),
                    });

                    slides.push(slide);
                }
            }
            PostDomain::Reddit => {
                for carousel_item in p.select(&img_selector) {
                    if let Some(url) = carousel_item.attr("src") {
                        let slide = Slide::Photo(Photo {
                            url: url.to_owned(),
                            site: PostDomain::Reddit,
                            created_timestamp: DateTime::parse_from_str(timestamp, format)?,
                            author: author.to_owned(),
                            author_id: author_id.to_owned(),
                            permalink: permalink.to_owned(),
                        });

                        slides.push(slide);
                    }
                }
            }
            PostDomain::Unknown(d) => {
                eprintln!("skipped a post from an unknown domain: {}", d);
            }
        }
    }

    Ok(())
}

fn get_next_navigation_url(document: &Html) -> Option<&str> {
    let selector = Selector::parse(r#"[id^="partial-more-posts-"]"#).unwrap();

    let more_posts_tag = document.select(&selector).next_back()?;

    more_posts_tag.attr("src")
}
