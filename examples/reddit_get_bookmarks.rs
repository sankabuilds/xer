use anyhow::{Context, Result};
use xxer::site::{
    common::Site,
    reddit::{Reddit, ViewType},
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let cookie_file = &args[1];

    let slides = Reddit::new(cookie_file)
        .get(ViewType::Bookmarks, None)
        .await
        .context("failed to get the ViewType")?;

    for slide in &slides {
        println!("{}: {}", slide, slide.get_file_name());
    }

    Ok(())
}
