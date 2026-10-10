use anyhow::{Context, Result};
use std::time::Duration;
use xxer::{
    downloader::reddit::DownloaderOptions,
    site::{
        common::Site,
        reddit::{Reddit, ViewType},
    },
};

use crate::args::{Cli, RedditBookmarksArgs};

pub async fn bookmarks(reddit_bookmarks_args: &RedditBookmarksArgs, args: &Cli) -> Result<()> {
    if let Some(cookie_file) = &args.cookie {
        let slides = if reddit_bookmarks_args.all {
            eprintln!("Gathering all your bookmarks. This may take some time!");

            Reddit::new(cookie_file)
                .get(ViewType::Bookmarks, None)
                .await
                .context("failed to get the ViewType")?
        } else {
            Reddit::new(cookie_file)
                .get(ViewType::Bookmarks, Some(reddit_bookmarks_args.limit))
                .await
                .context("failed to get the ViewType")?
        };

        DownloaderOptions::new()
            .timeout(Duration::from_millis(reddit_bookmarks_args.timeout))
            .download(slides, Some(reddit_bookmarks_args.thread_count))
            .await;
    } else {
        anyhow::bail!("Site requires a cookie file. see --help");
    }

    Ok(())
}
