use anyhow::{Context, Result};
use xxer::site::common::Site;
use xxer::site::instagram::{Instagram, ViewType};

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let cookie_file = &args[1];

    let slides = Instagram::new(cookie_file)
        .get(ViewType::Bookmarks, Some(100))
        .await
        .context("faild to get the ViewType")?;

    for slide in slides {
        println!("{slide}");
    }

    Ok(())
}
