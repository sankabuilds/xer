use std::{fmt::Display, path::Path};

use little_exif::{exif_tag::ExifTag, metadata::Metadata};
use mp4ameta::{Data, FreeformIdent};
use thiserror::Error;

pub trait Site {
    type ViewType;
    type Slide;
    type Error;

    fn new(cookie_file: &str) -> Self;

    fn get(
        &self,
        t: Self::ViewType,
        limit: Option<u32>,
    ) -> impl std::future::Future<Output = Result<Vec<Self::Slide>, Self::Error>> + Send;
}

pub enum Quality {
    Best,
    Mid,
    Low,
}

pub enum Slide<P, V> {
    Photo(P),
    Video(V),
}

#[derive(Error, Debug)]
pub enum MetadataError {
    #[error("failed to write metadata to the file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("mp4ameta error: {0}")]
    Mp4AmetaError(#[from] mp4ameta::Error),
}

pub trait WriteMetadata {
    fn write_metadata<P: AsRef<Path>>(&self, file_path: P) -> Result<(), MetadataError>;
}

pub struct ImageDescription<'a> {
    /// Try to follow this format: `<Name> (<handle>): Elon Musk (elonmusk)`
    pub author: &'a str,
    pub post_url: &'a str,
    /// People who are tagged by the author in a specific post (ex: in an instagram post)
    ///
    /// Try to follow this format: `<Name> (<handle>): 𝖦𝗋𝗂𝗆𝖾𝗌 ⏳ (Grimezsz)`
    pub tags: Option<Vec<String>>,
}

impl Display for ImageDescription<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(tags) = &self.tags {
            write!(
                f,
                "Post URL: {} Author: {} Tags: {:?}",
                self.post_url, self.author, tags
            )
        } else {
            write!(f, "Post URL: {} Author: {}", self.post_url, self.author)
        }
    }
}

pub fn w_photo_metadata(img_path: &Path, desc: ImageDescription) -> Result<(), MetadataError> {
    let mut metadata = Metadata::new();

    metadata.set_tag(ExifTag::ImageDescription(format!("{}", desc)));

    metadata.write_to_file(img_path)?;

    Ok(())
}

type StaticFreeFormIndent<'a> = mp4ameta::FreeformIdent<'a, mp4ameta::ident::StaticStr<'a>>;

const MEAN: &str = "com.github.sankabuilds.xer";
const POST_URL_IDENT: StaticFreeFormIndent = FreeformIdent::new_static(MEAN, "Post URL");
const AUTHOR_IDENT: StaticFreeFormIndent = FreeformIdent::new_static(MEAN, "Author");
const TAGS_IDENT: StaticFreeFormIndent = FreeformIdent::new_static(MEAN, "Tags");

pub enum VideoMetadataTag<'a> {
    /// Try to follow this format: `<Name> (<handle>): Elon Musk (elonmusk)`
    Author(&'a str),
    PostUrl(&'a str),
    /// People who are tagged by the author in a specific post (ex: in an instagram post)
    ///
    /// Try to follow this format: `<Name> (<handle>): 𝖦𝗋𝗂𝗆𝖾𝗌 ⏳ (Grimezsz)`
    Tags(Option<Vec<&'a str>>),
}

pub fn w_video_metadata(vid_path: &Path, tags: Vec<VideoMetadataTag>) -> Result<(), MetadataError> {
    let mut video_tags = mp4ameta::Tag::read_from_path(vid_path)?;

    for tag in tags {
        match tag {
            VideoMetadataTag::Author(a) => {
                video_tags.add_data(AUTHOR_IDENT, mp4ameta::Data::Utf8(a.into()))
            }
            VideoMetadataTag::PostUrl(p) => {
                video_tags.add_data(POST_URL_IDENT, mp4ameta::Data::Utf8(p.into()));
            }
            VideoMetadataTag::Tags(tags) => {
                if let Some(tags_vec) = tags {
                    for tag in tags_vec {
                        video_tags.add_data(TAGS_IDENT, Data::Utf8(tag.into()));
                    }
                }
            }
        }
    }

    video_tags.write_to_path(vid_path)?;

    Ok(())
}
