use anyhow::Result;
use xxer::downloader::common::request_hls;

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();

    if args.len() >= 2 {
        let debug = &args[1];
        if !debug.is_empty() {
            tracing_subscriber::fmt()
                .with_env_filter("xxer=trace,test_reddit_hls_request=trace")
                .init();
        }
    }

    let url = "https://v.redd.it/18hzpuvzdnrh1/HLSPlaylist.m3u8";

    let out_path = "hls_video.mp4";

    request_hls(url, out_path, None).await?;
    Ok(())
}
