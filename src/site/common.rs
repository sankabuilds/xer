use std::path::Path;

use little_exif::{exif_tag::ExifTag, metadata::Metadata};
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

pub fn w_photo_metadata(img_path: &Path, tags: Vec<ExifTag>) -> Result<(), MetadataError> {
    let mut metadata = Metadata::new();

    for tag in tags {
        metadata.set_tag(tag);
    }

    metadata.write_to_file(img_path)?;

    Ok(())
}

pub enum VideoMetadataTag<'a> {
    Artist(&'a str),
}

pub fn w_video_metadata(vid_path: &Path, tags: Vec<VideoMetadataTag>) -> Result<(), MetadataError> {
    let mut video_tags = mp4ameta::Tag::read_from_path(vid_path)?;

    for tag in tags {
        match tag {
            VideoMetadataTag::Artist(a) => video_tags.set_artist(a),
        }
    }

    video_tags.write_to_path(vid_path)?;

    Ok(())
}
