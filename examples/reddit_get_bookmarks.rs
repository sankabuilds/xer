use anyhow::{Context, Result};
use xxer::site::{
    common::Site,
    reddit::{Reddit, ViewType},
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let cookie_file = &args[1];

    if args.len() >= 3 {
        let debug = &args[2];
        if !debug.is_empty() {
            tracing_subscriber::fmt()
                .with_env_filter("xxer=trace,get=trace")
                .init();
        }
    }

    let slides = Reddit::new(cookie_file)
        .get(ViewType::Bookmarks, Some(100))
        .await
        .context("failed to get the ViewType")?;

    for slide in &slides {
        println!("{}: {}", slide, slide.get_file_name());
    }

    Ok(())
}
