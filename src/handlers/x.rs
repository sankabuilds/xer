use anyhow::{Context, Result};
use std::{io::Write, time::Duration};

use xxer::{
    downloader::x::DownloaderOptions,
    site::x::{Slide, ViewType, XTwitter},
};

use crate::args::{Cli, XBookmarksArgs};

pub async fn bookmarks(x_bookmarks_args: &XBookmarksArgs, args: &Cli) -> Result<()> {
    if let Some(cookie_file) = &args.cookie {
        let slides: Vec<Slide>;

        if x_bookmarks_args.all {
            print!("Gathering all your bookmarks. This may take some time!");
            std::io::stdout().flush()?;

            slides = XTwitter::new(cookie_file)
                .get(ViewType::Bookmarks, None)
                .await
                .map(|res: Vec<Slide>| {
                    print!("\r\x1B[2K");

                    res
                })
                .context("failed to get the ViewType")?;
        } else {
            slides = XTwitter::new(cookie_file)
                .get(ViewType::Bookmarks, Some(x_bookmarks_args.limit))
                .await
                .context("failed to get the ViewType")?;
        }

        DownloaderOptions::new()
            .timeout(Duration::from_millis(100))
            .download(&slides)
            .await;
    } else {
        anyhow::bail!("Site requires a cookie file. see --help");
    }

    Ok(())
}
