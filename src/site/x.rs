#![allow(dead_code)]

use indicatif::MultiProgress;
use log::info;
use reqwest::StatusCode;
use reqwest::Url;
use reqwest::cookie::Jar;
use serde::Deserialize;
use serde_json::Value;
use std::fmt::Display;
use std::sync::Arc;
use thiserror::Error;

use crate::cookie;
use crate::cookie::x::XCookieError;
use crate::downloader;
use crate::downloader::common::CommonDownloaderError;
use crate::site::common;
use crate::site::common::ImageDescription;
use crate::site::common::MetadataError;
use crate::site::common::Quality;
use crate::site::common::VideoMetadataTag;
use crate::site::common::WriteMetadata;
use crate::site::common::w_photo_metadata;
use crate::site::common::w_video_metadata;

fn format_msg(msg: Option<&String>) -> String {
    match msg {
        Some(msg) => msg.to_owned(),
        None => "Empty".to_string(),
    }
}

#[derive(Error, Debug)]
pub enum XError {
    #[error("HTTP request failed: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("You have zero bookmarks")]
    BookmarksEmpty,

    #[error("Failed to parse a slide Value: {value}. serde error: {err}")]
    SlideParseFailed {
        value: Value,
        err: serde_json::Error,
    },

    #[error("HTTP request failed. Status code: {status_code}. Response Body: {0}", format_msg(.msg.as_ref()))]
    HTTPRequestFailed {
        status_code: StatusCode,
        msg: Option<String>,
    },

    #[error("serde_json operation failed")]
    SerdeJson(#[from] serde_json::Error),

    #[error("returned an unexpected response shape: {path}: {msg}")]
    UnexpectedResponseShape { path: String, msg: String },

    #[error("Http request preparation failed: {0}")]
    CookieError(#[from] XCookieError),

    #[error("Failed to extract meta data from the Value: {0}")]
    MetadataParsingError(Value),
}

#[derive(Deserialize, Debug)]
struct PhotoDto {
    #[serde(rename = "type")]
    slide_type: String,
    media_url_https: String,
    id_str: String,
    expanded_url: String,
}

impl PhotoDto {
    pub fn into_photo(self, sort_index: &str, author: &str, author_screen_name: &str) -> Photo {
        Photo {
            slide_type: self.slide_type,
            media_url_https: self.media_url_https,
            id_str: self.id_str,
            metadata: MetadataContainer {
                sort_index: sort_index.to_owned(),
                slide_url: self.expanded_url,
                author: author.to_owned(),
                author_screen_name: author_screen_name.to_owned(),
            },
        }
    }
}

pub struct Photo {
    pub slide_type: String,
    pub media_url_https: String,
    pub id_str: String,

    pub metadata: MetadataContainer,
}

impl Display for Photo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.media_url_https)
    }
}

impl Photo {
    fn get_file_name(&self) -> String {
        let name = format!("{}_{}", self.metadata.sort_index, self.id_str);
        let ext = self
            .media_url_https
            .parse::<Url>()
            .ok()
            .and_then(|url| {
                url.path_segments()
                    .and_then(|mut segments| segments.next_back())
                    .and_then(|filename| filename.rsplit_once('.').map(|(_, ext)| ext.to_string()))
            })
            .unwrap_or_else(|| "bin".to_string());

        format!("{name}.{ext}")
    }
}

#[derive(Deserialize, Debug)]
struct VideoDto {
    #[serde(rename = "type")]
    slide_type: String,
    media_url_https: String,
    id_str: String,
    expanded_url: String,
    video_info: VideoInfo,
}

impl VideoDto {
    pub fn into_video(self, sort_index: &str, author: &str, author_screen_name: &str) -> Video {
        Video {
            slide_type: self.slide_type,
            media_url_https: self.media_url_https,
            id_str: self.id_str,
            video_info: self.video_info,
            metadata: MetadataContainer {
                sort_index: sort_index.to_owned(),
                slide_url: self.expanded_url,
                author: author.to_owned(),
                author_screen_name: author_screen_name.to_owned(),
            },
        }
    }
}

pub struct Video {
    pub slide_type: String,
    pub media_url_https: String,
    pub id_str: String,
    pub video_info: VideoInfo,

    pub metadata: MetadataContainer,
}

pub struct MetadataContainer {
    pub sort_index: String,
    pub slide_url: String,
    pub author: String,
    pub author_screen_name: String,
}

impl Video {
    fn get_file_name(&self) -> String {
        let name = format!("{}_{}", self.metadata.sort_index, self.id_str);
        let ext = self
            .video_info
            .get(Quality::Best)
            .url
            .parse::<Url>()
            .ok()
            .and_then(|url| {
                url.path_segments()
                    .and_then(|mut segments| segments.next_back())
                    .and_then(|filename| filename.rsplit_once('.').map(|(_, ext)| ext.to_string()))
            })
            .unwrap_or_else(|| "bin".to_string());

        format!("{name}.{ext}")
    }
}

impl Display for Video {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.video_info.get(Quality::Best).url)
    }
}

#[derive(Deserialize, Debug)]
pub struct VideoInfo {
    aspect_ratio: Vec<i32>,
    duration_millis: i32,
    variants: Vec<Variant>,
}

impl VideoInfo {
    pub fn get(&self, quality: Quality) -> &Variant {
        let mut v: Vec<&Variant> = self
            .variants
            .iter()
            .filter(|i| i.bitrate.is_some())
            .collect();

        v.sort_by_key(|a| a.bitrate.unwrap());

        match quality {
            Quality::Best => v[v.len() - 1],
            _ => unimplemented!(),
        }
    }
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Variant {
    pub bitrate: Option<usize>,
    pub content_type: String,
    pub url: String,
}

pub type Slide = common::Slide<Photo, Video>;

impl Slide {
    pub async fn download(&self, m_pb: Option<MultiProgress>) -> Result<(), CommonDownloaderError> {
        downloader::x::fetch(self, m_pb).await
    }

    pub fn get_file_name(&self) -> String {
        match self {
            Slide::Photo(photo) => photo.get_file_name(),
            Slide::Video(video) => video.get_file_name(),
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

impl WriteMetadata for Slide {
    fn write_metadata<P: AsRef<std::path::Path>>(&self, file_path: P) -> Result<(), MetadataError> {
        match self {
            Self::Photo(p) => {
                let author = format!("{} (@{})", p.metadata.author, p.metadata.author_screen_name);

                w_photo_metadata(
                    file_path.as_ref(),
                    ImageDescription {
                        author: &author,
                        post_url: &p.metadata.slide_url,
                        tags: None,
                    },
                )
            }
            Self::Video(v) => {
                let author = format!("{} (@{})", v.metadata.author, v.metadata.author_screen_name);

                let tags = vec![
                    VideoMetadataTag::PostUrl(&v.metadata.slide_url),
                    VideoMetadataTag::Author(&author),
                ];

                w_video_metadata(file_path.as_ref(), tags)
            }
        }
    }
}

enum SlideType {
    Photo,
    Video,
    Unknown,
}

pub enum ViewType {
    Bookmarks,
}

pub struct XTwitter {
    client: reqwest::Client,
    cookie_jar: Arc<Jar>,
}

enum Limit {
    Max { slide_count: u32 },
    All,
}

impl XTwitter {
    pub fn new(cookie_file: &str) -> Self {
        let jar = Arc::new(cookie::x::get_jar(cookie_file));

        Self {
            client: cookie::x::new_loaded_client(Arc::clone(&jar)),
            cookie_jar: jar,
        }
    }

    /// `limit` doesn't guarantee the length of the returned `Vec<Slide>`.
    ///
    /// For a `ViewType` like `Bookmarks`, it always will be more than or equal, unless
    /// the user's account doesn't have bookmarks more than that.
    pub async fn get(&self, t: ViewType, limit: Option<u32>) -> Result<Vec<Slide>, XError> {
        let slide_limit = limit
            .map(|slide_count| Limit::Max { slide_count })
            .unwrap_or(Limit::All);

        match t {
            ViewType::Bookmarks => self.get_bookmarks(&slide_limit).await,
        }
    }

    async fn get_bookmarks(&self, limit: &Limit) -> Result<Vec<Slide>, XError> {
        info!("Getting bookmarks!");

        let mut cursor: Option<String> = None;

        let mut slides_arr: Vec<Slide> = vec![];

        loop {
            if let Limit::Max { slide_count } = limit
                && slides_arr.len() >= *slide_count as usize
            {
                break;
            }

            // count 100 is the maximum amount we can request at a single time
            let variables = {
                if let Some(c) = &cursor {
                    format!(r#"{{"count":100,"cursor":"{c}","includePromotedContent":true}}"#)
                } else {
                    r#"{"count":100,"includePromotedContent":true}"#.to_string()
                }
            };

            let features = r#"{"rweb_video_screen_enabled":false,"rweb_cashtags_enabled":true,"profile_label_improvements_pcf_label_in_post_enabled":true,"responsive_web_profile_redirect_enabled":false,"rweb_tipjar_consumption_enabled":false,"verified_phone_label_enabled":false,"creator_subscriptions_tweet_preview_api_enabled":true,"responsive_web_graphql_timeline_navigation_enabled":true,"responsive_web_graphql_skip_user_profile_image_extensions_enabled":false,"premium_content_api_read_enabled":false,"communities_web_enable_tweet_community_results_fetch":true,"c9s_tweet_anatomy_moderator_badge_enabled":true,"responsive_web_grok_analyze_button_fetch_trends_enabled":false,"responsive_web_grok_analyze_post_followups_enabled":true,"rweb_cashtags_composer_attachment_enabled":true,"responsive_web_jetfuel_frame":true,"responsive_web_grok_share_attachment_enabled":true,"responsive_web_grok_annotations_enabled":true,"articles_preview_enabled":true,"responsive_web_edit_tweet_api_enabled":true,"rweb_conversational_replies_downvote_enabled":false,"graphql_is_translatable_rweb_tweet_is_translatable_enabled":true,"view_counts_everywhere_api_enabled":true,"longform_notetweets_consumption_enabled":true,"responsive_web_twitter_article_tweet_consumption_enabled":true,"content_disclosure_indicator_enabled":true,"content_disclosure_ai_generated_indicator_enabled":true,"responsive_web_grok_show_grok_translated_post":true,"responsive_web_grok_analysis_button_from_backend":true,"post_ctas_fetch_enabled":false,"freedom_of_speech_not_reach_fetch_enabled":true,"standardized_nudges_misinfo":true,"tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled":true,"longform_notetweets_rich_text_read_enabled":true,"longform_notetweets_inline_media_enabled":false,"responsive_web_grok_image_annotation_enabled":true,"responsive_web_grok_imagine_annotation_enabled":true,"responsive_web_grok_community_note_auto_translation_is_enabled":true,"responsive_web_enhance_cards_enabled":false}"#;

            let csrf_token = cookie::x::get_csrf_token(Arc::clone(&self.cookie_jar))?;
            let req = self.client
        .get("https://x.com/i/api/graphql/tUVliYsHyxrQIT4HXUWNdA/Bookmarks")
        .query(&[("variables", variables), ("features", features.to_string())])
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36").header("Content-Type", "application/json").header("X-Csrf-Token", csrf_token).header("Authorization", "Bearer AAAAAAAAAAAAAAAAAAAAANRILgAAAAAAnNwIzUejRCOuH5E6I8xnZz4puTs%3D1Zv7ttfk8LF81IUq16cHjhLTvJu4FA33AGWWjCpTnA");

            let res = req.send().await?;

            let status = res.status();
            if status != reqwest::StatusCode::OK {
                return Err(XError::HTTPRequestFailed {
                    status_code: status,
                    msg: Some(res.text().await?),
                });
            }

            let path = res.url().path().to_string();
            let bytes = res.bytes().await?;

            let deserialized: Value = serde_json::from_slice(&bytes)?;

            let entries_vec = deserialized["data"]["bookmark_timeline_v2"]["timeline"]["instructions"]
            [0]["entries"]
            .as_array()
            .ok_or_else(|| XError::UnexpectedResponseShape {
                path: path.clone(),
                msg: "bookmarks response doesn't contain an entries array".into()
            })?;

            if entries_vec.len() == 2 {
                break;
            } else {
                let next_cursor =
                    &entries_vec
                        .last()
                        .ok_or_else(|| XError::UnexpectedResponseShape {
                            path: path.clone(),
                            msg: "couldn't access the last entry".into(),
                        })?["content"];

                #[derive(Deserialize, Debug, PartialEq)]
                #[serde(rename_all = "camelCase")]
                struct TimelineCursor {
                    #[serde(rename = "__typename")]
                    type_name: String,
                    cursor_type: String,
                    entry_type: String,
                    stop_on_empty_response: bool,
                    value: String,
                }

                let timeline_cursor: TimelineCursor =
                    serde_json::from_value(next_cursor.clone()).map_err(|_| XError::UnexpectedResponseShape { path: path.clone(), msg: "serde_json operation failed. failed to parse the Value into a TimelineCursor".into() })?;

                if timeline_cursor.cursor_type == "Bottom" {
                    cursor = Some(timeline_cursor.value);
                } else {
                    return Err(XError::UnexpectedResponseShape {
                        path: path.clone(),
                        msg: "cursor_type is not Bottom".into(),
                    });
                }
            }

            for entry in entries_vec {
                let mut slides = &entry["content"]["itemContent"]["tweet_results"]["result"]["legacy"]
                    ["entities"]["media"];

                if !slides.is_array() {
                    // Some tweets have this json path
                    slides = &entry["content"]["itemContent"]["tweet_results"]["result"]["tweet"]["legacy"]
                        ["entities"]["media"];
                }

                if let Some(arr) = slides.as_array() {
                    let mut core = &entry["content"]["itemContent"]["tweet_results"]["result"]["core"]
                        ["user_results"]["result"]["core"];

                    if !core.is_object() {
                        core = &entry["content"]["itemContent"]["tweet_results"]["result"]["tweet"]
                            ["core"]["user_results"]["result"]["core"];
                    }

                    let author = core["name"]
                        .as_str()
                        .ok_or_else(|| XError::MetadataParsingError(entry.clone()))?;

                    let author_username = core["screen_name"]
                        .as_str()
                        .ok_or_else(|| XError::MetadataParsingError(entry.clone()))?;

                    for slide in arr {
                        let slide_type = {
                            if let Some(t) = slide["type"].as_str() {
                                match t {
                                    "photo" => SlideType::Photo,
                                    "video" => SlideType::Video,
                                    _ => SlideType::Unknown,
                                }
                            } else {
                                SlideType::Unknown
                            }
                        };

                        let sort_index = entry["sortIndex"].as_str().ok_or_else(|| {
                            XError::UnexpectedResponseShape {
                                path: path.clone(),
                                msg: "every entry should have a sortIndex".into(),
                            }
                        })?;

                        match slide_type {
                            SlideType::Photo => {
                                let dto: PhotoDto = {
                                    match serde_json::from_value(slide.clone()) {
                                        Ok(k) => k,
                                        Err(err) => {
                                            return Err(XError::SlideParseFailed {
                                                value: slide.clone(),
                                                err,
                                            });
                                        }
                                    }
                                };

                                let photo = dto.into_photo(sort_index, author, author_username);

                                slides_arr.push(Slide::Photo(photo));
                            }
                            SlideType::Video => {
                                let dto: VideoDto = {
                                    match serde_json::from_value(slide.clone()) {
                                        Ok(k) => k,
                                        Err(err) => {
                                            return Err(XError::SlideParseFailed {
                                                value: slide.clone(),
                                                err,
                                            });
                                        }
                                    }
                                };

                                let video = dto.into_video(sort_index, author, author_username);

                                slides_arr.push(Slide::Video(video));
                            }
                            SlideType::Unknown => {
                                continue;
                            }
                        }
                    }
                }
            }

            info!("Slide count: {}", slides_arr.len());
        }

        if slides_arr.is_empty() {
            return Err(XError::BookmarksEmpty);
        }

        Ok(slides_arr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_video_info() -> VideoInfo {
        let variants: Vec<Variant> = vec![
            Variant {
                bitrate: None,
                content_type: "application/x-mpegURL".to_string(),
                url: "https://video.twimg.com/amplify_video/2065494644855603201/pl/TJEvIdzWNSLTvI3y.m3u8?tag=27&v=cfc".to_string(),
            },
            Variant {
                bitrate: Some(632000),
                content_type: "video/mp4".to_string(),
                url: "https://video.twimg.com/amplify_video/2065494644855603201/vid/avc1/320x568/QxnLlh08mXHLDiWk.mp4?tag=27".to_string(),
            },
            Variant {
                bitrate: Some(950000),
                content_type: "video/mp4".to_string(),
                url: "https://video.twimg.com/amplify_video/2065494644855603201/vid/avc1/480x852/JmTIKwvAA3p8g5jv.mp4?tag=27".to_string(),
            },
            Variant {
                bitrate: Some(2176000),
                content_type: "video/mp4".to_string(),
                url: "https://video.twimg.com/amplify_video/2065494644855603201/vid/avc1/720x1280/aJuyc0Egotbt0AX5.mp4?tag=27".to_string(),
            },
        ];

        VideoInfo {
            aspect_ratio: vec![9, 16],
            duration_millis: 36566,
            variants,
        }
    }

    #[test]
    fn test_videoinfo_get() {
        let video_info = get_video_info();

        let best_variant = Variant{
                bitrate: Some(2176000),
                content_type: "video/mp4".to_string(),
                url: "https://video.twimg.com/amplify_video/2065494644855603201/vid/avc1/720x1280/aJuyc0Egotbt0AX5.mp4?tag=27".to_string(),
        };

        assert_eq!(video_info.get(Quality::Best), &best_variant);
    }

    #[test]
    #[should_panic]
    fn test_videoinfo_get_panic() {
        let video_info = get_video_info();

        video_info.get(Quality::Low);
    }
}
