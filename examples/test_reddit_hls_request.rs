use anyhow::Result;
use xxer::downloader::common::request_hls;

#[tokio::main]
async fn main() -> Result<()> {
    let url = "https://v.redd.it/18hzpuvzdnrh1/HLSPlaylist.m3u8";

    let out_path = "hls_video.mp4";

    request_hls(url, out_path, None).await?;
    Ok(())
}
