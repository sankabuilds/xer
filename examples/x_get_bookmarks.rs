use anyhow::{Context, Result};
use xxer::site::x::{ViewType, XTwitter};

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let cookie_file = &args[1];

    let slides = XTwitter::new(cookie_file)
        .get(ViewType::Bookmarks, Some(100))
        .await
        .context("failed to get the ViewType")?;

    for slide in slides {
        println!("{slide}");
    }

    Ok(())
}
