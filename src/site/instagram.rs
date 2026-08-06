#![allow(dead_code)]

use indicatif::MultiProgress;
use log::info;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt::Display, str::FromStr, sync::Arc};
use thiserror::Error;

use crate::{
    cookie::instagram::{get_jar, new_loaded_client},
    downloader::{self, common::CommonDownloaderError},
    site::common::{Quality, Site},
};

pub const INSTAGRAM: &str = "https://www.instagram.com";

#[derive(Error, Debug)]
pub enum InstagramError {
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("Unexpected response: {0}")]
    UnexpectedResponse(String),

    #[error("Unknown product type: {p_type}\n\nItem: {item:?}")]
    UnknownProductType { p_type: String, item: Value },
}

#[derive(Debug)]
pub enum Slide {
    Photo(Photo),
    Video(Video),
}

impl From<Clip> for Slide {
    fn from(c: Clip) -> Self {
        Self::Video(Video {
            parent_pk: None,
            pk: c.pk.clone(),
            url: c.video_version_container.get(Quality::Best).to_owned(),
        })
    }
}

impl From<Feed> for Slide {
    fn from(f: Feed) -> Self {
        match f.feed_container {
            FeedContainer::Photo { image_versions2 } => Self::Photo(Photo {
                parent_pk: None,
                pk: f.pk,
                url: image_versions2.get(Quality::Best).to_owned(),
            }),
            FeedContainer::Video(v) => Self::Video(Video {
                parent_pk: None,
                pk: f.pk,
                url: v.get(Quality::Best).to_owned(),
            }),
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
        downloader::instagram::fetch(self, m_pb).await
    }
}

#[derive(Debug)]
pub struct Photo {
    pub parent_pk: Option<String>,
    pub pk: String,
    pub url: String,
}

impl Display for Photo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.url)
    }
}

impl Photo {
    fn get_file_name(&self) -> String {
        let ext = self
            .url
            .parse::<Url>()
            .ok()
            .and_then(|url| {
                url.path_segments()
                    .and_then(|mut segments| segments.next_back())
                    .and_then(|file_name| file_name.rsplit_once("."))
                    .map(|(_, ext)| ext.to_string())
            })
            .unwrap_or_else(|| "bin".to_string());

        if let Some(p_pk) = &self.parent_pk {
            format!("{}_{}.{}", p_pk, self.pk, ext)
        } else {
            format!("{}.{}", self.pk, ext)
        }
    }
}

#[derive(Debug)]
pub struct Video {
    pub parent_pk: Option<String>,
    pub pk: String,
    pub url: String,
}

impl Display for Video {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.url)
    }
}

impl Video {
    fn get_file_name(&self) -> String {
        let ext = self
            .url
            .parse::<Url>()
            .ok()
            .and_then(|url| {
                url.path_segments()
                    .and_then(|mut segments| segments.next_back())
                    .and_then(|file_name| file_name.rsplit_once("."))
                    .map(|(_, ext)| ext.to_string())
            })
            .unwrap_or_else(|| "bin".to_string());

        if let Some(p_pk) = &self.parent_pk {
            format!("{}_{}.{}", p_pk, self.pk, ext)
        } else {
            format!("{}.{}", self.pk, ext)
        }
    }
}

pub enum ViewType {
    Bookmarks,
}

pub struct Instagram {
    client: reqwest::Client,
}

impl Site for Instagram {
    type Error = InstagramError;
    type Slide = Slide;
    type ViewType = ViewType;

    fn new(cookie_file: &str) -> Self {
        let jar = Arc::new(get_jar(cookie_file));

        Self {
            client: new_loaded_client(Arc::clone(&jar)),
        }
    }

    async fn get(
        &self,
        t: Self::ViewType,
        limit: Option<u32>,
    ) -> Result<Vec<Self::Slide>, Self::Error> {
        let slide_limit = limit
            .map(|slide_count| Limit::Max { slide_count })
            .unwrap_or(Limit::All);

        match t {
            ViewType::Bookmarks => self.get_bookmarks(&slide_limit).await,
        }
    }
}

#[derive(Debug)]
enum ProductType {
    /// A single video
    Clips,
    /// Multiple items. Can contain combination of both videos & images
    CarouselContainer,
    /// A single image/video
    Feed,
    /// Ad
    Ad,
    Igtv,
}

/// ProductType::Clips
#[derive(Serialize, Deserialize)]
struct Clip {
    #[serde(skip_deserializing)]
    pk: String,

    code: String,
    #[serde(flatten)]
    video_version_container: VideoVersionContainer,
}

#[derive(Serialize, Deserialize, Debug)]
struct VideoVersion {
    bandwidth: u32,
    height: u16,
    width: u16,
    #[serde(rename = "type")]
    v_type: u8,
    url: String,
}

/// ProductType::CarouselContainer
#[derive(Serialize, Deserialize)]
struct CarouselContainer {
    code: String,
    carousel_media: Vec<CarouselItem>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum CarouselItem {
    Video {
        product_type: String,
        media_type: u8,
        #[serde(flatten)]
        video_version_container: VideoVersionContainer,
        pk: String,
    },
    Image {
        product_type: String,
        media_type: u8,
        pk: String,
        image_versions2: CandidateContainer,
    },
}

#[derive(Serialize, Deserialize, Debug)]
struct VideoVersionContainer {
    video_versions: Vec<VideoVersion>,
}

impl VideoVersionContainer {
    fn get(&self, q: Quality) -> &str {
        match q {
            Quality::Best => &self.video_versions[0].url,
            Quality::Mid => unimplemented!(),
            Quality::Low => unimplemented!(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct CandidateContainer {
    candidates: Vec<CandidateItem>,
}

impl CandidateContainer {
    fn get(&self, q: Quality) -> &str {
        match q {
            Quality::Best => &self.candidates[0].url,
            Quality::Mid => unimplemented!(),
            Quality::Low => unimplemented!(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct CandidateItem {
    url: String,
    height: u16,
    width: u16,
}

/// ProductType::Feed
#[derive(Serialize, Deserialize, Debug)]
struct Feed {
    #[serde(skip_deserializing)]
    pk: String,

    #[serde(flatten)]
    feed_container: FeedContainer,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum FeedContainer {
    Video(VideoVersionContainer),
    Photo { image_versions2: CandidateContainer },
}

enum Limit {
    Max { slide_count: u32 },
    All,
}

impl Instagram {
    async fn get_bookmarks(&self, limit: &Limit) -> Result<Vec<Slide>, InstagramError> {
        info!("Getting bookmarks!");
        let mut slide_list: Vec<Slide> = Vec::new();
        let mut next_max_id: Option<String> = None;

        loop {
            match limit {
                Limit::All => {}
                Limit::Max { slide_count } => {
                    if slide_list.len() >= *slide_count as usize {
                        break;
                    }
                }
            }

            let mut req = self
            .client
            .get(INSTAGRAM.to_owned() + "/api/v1/feed/saved/posts/")
            .header("X-Ig-App-Id", "936619743392459")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36");

            if let Some(next_id) = next_max_id {
                req = req.query(&[("max_id", next_id)]);
            }

            let res = req.send().await?;

            let body = res.text().await?;
            let res = serde_json::Value::from_str(&body)?;

            for item in res["items"].as_array().ok_or_else(|| {
                InstagramError::UnexpectedResponse("could not find `items` array".to_owned())
            })? {
                let item_type: ProductType =
                    match item["media"]["product_type"].as_str().ok_or_else(|| {
                        InstagramError::UnexpectedResponse(
                            "could not find `product_type` in the item".to_owned(),
                        )
                    })? {
                        "clips" => ProductType::Clips,
                        "carousel_container" => ProductType::CarouselContainer,
                        "feed" => ProductType::Feed,
                        "ad" => ProductType::Ad,
                        "igtv" => ProductType::Igtv,
                        unknown => {
                            return Err(InstagramError::UnknownProductType {
                                p_type: unknown.to_owned(),
                                item: item.clone(),
                            });
                        }
                    };

                let p_pk = item["media"]["pk"].as_str().ok_or_else(|| {
                    InstagramError::UnexpectedResponse("could not find `pk` in the item".to_owned())
                })?;

                match item_type {
                    ProductType::Clips => {
                        let mut clip = serde_json::from_value::<Clip>(item["media"].clone())?;
                        clip.pk = p_pk.to_owned();

                        slide_list.push(clip.into());
                    }
                    ProductType::CarouselContainer => {
                        let carousel_media =
                            serde_json::from_value::<CarouselContainer>(item["media"].clone())?;

                        for carousel_item in carousel_media.carousel_media {
                            match carousel_item {
                                CarouselItem::Image {
                                    product_type: _,
                                    media_type: _,
                                    image_versions2,
                                    pk,
                                } => {
                                    let photo: Photo = Photo {
                                        parent_pk: Some(p_pk.to_string()),
                                        pk,
                                        url: image_versions2.get(Quality::Best).to_owned(),
                                    };

                                    slide_list.push(Slide::Photo(photo));
                                }
                                CarouselItem::Video {
                                    product_type: _,
                                    media_type: _,
                                    pk,
                                    video_version_container,
                                } => {
                                    let video: Video = Video {
                                        parent_pk: Some(p_pk.to_owned()),
                                        pk,
                                        url: video_version_container.get(Quality::Best).to_owned(),
                                    };

                                    slide_list.push(Slide::Video(video));
                                }
                            }
                        }
                    }
                    ProductType::Feed => {
                        let mut feed = serde_json::from_value::<Feed>(item["media"].clone())?;
                        feed.pk = p_pk.to_owned();

                        slide_list.push(feed.into());
                    }
                    ProductType::Ad => {
                        // TODO
                    }
                    ProductType::Igtv => {
                        // TODO
                    }
                }
            }

            info!("Slide count: {}", slide_list.len());

            if res["more_available"].as_bool().ok_or_else(|| {
                InstagramError::UnexpectedResponse(
                    "could not find `more_available` field in the response".to_owned(),
                )
            })? {
                next_max_id = Some(
                    res["next_max_id"]
                        .as_str()
                        .ok_or_else(|| {
                            InstagramError::UnexpectedResponse(
                                "could not find `next_max_id` field in the response".to_owned(),
                            )
                        })?
                        .to_owned(),
                );
            } else {
                break;
            }
        }

        Ok(slide_list)
    }
}

#[cfg(test)]
mod tests {
    use crate::site::instagram::Photo;

    #[test]
    fn test_get_file_name() {
        let photo: Photo = Photo {
            parent_pk: None,
            pk: String::from("3938716240889371107"),
            url: String::from(
                "https://instagram.fcmb3-2.fna.fbcdn.net/o1/v/t2/f2/m86/AQPXRRddBe2JJvoH1tkqbcCbClRrUAvH_fu0ZNeuhMWzDILYM-9hUvy9HU19g044dFtQs.mp4?_nc_cat=1039&_nc_oc=AddobZtPa56R2FE4vbiRQ9ItVVq8T4rwVQ5CgLdXN15NBdxBWH91DnresolFYniKMdjQ&_nc_sid=5e9851&_nc_ht=instagram.fcmb3-2.fna.fbcdn.net&_nc_ohc=MEb55SECVPIQ7kNvwHtinuz&efg=eyJ2ZW5jb2RlX3RhZyI6Inhwdl9wcm9ncmVzc2l2ZS5JTlNUQUdSQU0uQ0xJUFMuQzIuNzIwLmRhc2hfYmFzZWxpbmVfMV92MSIsInhwdl9hc3NldF9pZCI6MTA1dfgMzkxNzYyMfferfgdfgDUwMDM2MywiYXNzZXRfYWdlX2RheXMiOjMsInZpX3VzZWNhc2VfaWQiOjEwMDk5LCJkdXJhdGlvbl9zIjo2NywidXJsZ2VuX3NvdXJjZSI6Ind3dyJ9&ccb=17-1&vs=6791aerer04de3620e3f&_nc_vs=HBksddFQIYUmertlnX3hwdl9yZWVsc19wZXJtYW5lbnRfc3JfcHJvZC9DMTQzRDRBNzQ5RUI1OTcxNkI5RjVGMkFBRjhFRUZBMV92aWRlb19kYXNoaW5pdC5tcDQVAALIARIAFQIYUWlnX3hwdl9wbGFjZW1lbnRfcGVybWFuZW50X3YyL0ZDNEMzOTY1Q0JGNTEyOEQ3MEZBNEIyMDcyMTY4NkJBX2F1ZGlvX2Rhc2hpbml0Lm1wNBUCAsgBEgAoABgwerweAGwKIB3VzZV9vaWwBMRJwcm9ncmVzc2l2ZV9yZWNpcGUBMRUAACaW3qOWiqLfAxUCKAJDMiwXQFDgAAAAAAAYEmRhc2hfYmFzZWxpbmVfMV92MREAdf4HZeadAQA&_nc_gid=3_iO4Il340bLyzPHoQCQgQ&_nc_ss=7a22e&_nc_map=urlgen_bucketless&_nc_zt=28&oh=00_AQD-bRxC-th9QoaOTtrtert1f2EU_5WWC9V5fYudd5xtgAh5BetaGA&oe=6A6Aert5ECE",
            ),
        };

        assert_eq!(photo.get_file_name(), "3938716240889371107.mp4");
    }
}
