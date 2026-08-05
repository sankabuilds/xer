use anyhow::{Context, Result};
use std::time::Duration;

use xxer::{
    downloader::x::DownloaderOptions,
    site::x::{ViewType, XTwitter},
};

use crate::args::{Cli, XBookmarksArgs};

pub async fn bookmarks(x_bookmarks_args: &XBookmarksArgs, args: &Cli) -> Result<()> {
    if let Some(cookie_file) = &args.cookie {
        let slides = if x_bookmarks_args.all {
            eprintln!("Gathering all your bookmarks. This may take some time!");

            XTwitter::new(cookie_file)
                .get(ViewType::Bookmarks, None)
                .await
                .context("failed to get the ViewType")?
        } else {
            XTwitter::new(cookie_file)
                .get(ViewType::Bookmarks, Some(x_bookmarks_args.limit))
                .await
                .context("failed to get the ViewType")?
        };

        DownloaderOptions::new()
            .timeout(Duration::from_millis(x_bookmarks_args.timeout))
            .download(slides, Some(x_bookmarks_args.thread_count))
            .await;
    } else {
        anyhow::bail!("Site requires a cookie file. see --help");
    }

    Ok(())
}
