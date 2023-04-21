use anyhow::{anyhow, Result};
use futures::future::try_join_all;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT_RANGES, CONTENT_LENGTH, RANGE};
use reqwest::Client;
use std::fs::File;
use std::io::{Seek, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

#[derive(Default)]
pub struct Downloader {
    client: Client,
}

impl Downloader {
    /// Checks if downloading url accepts content-range header
    pub async fn is_accept_ranges(&self, url: &str) -> Result<bool> {
        let response = self.client.head(url).send().await?;
        let header = response.headers().get(ACCEPT_RANGES);
        Ok(matches!(header, Some(value) if value == "bytes"))
    }

    /// Download file at url and save to save_to path
    pub async fn download_to(
        &self,
        url: &str,
        title: &str,
        save_to: PathBuf,
        threads: u64,
    ) -> Result<()> {
        let total_size = self
            .client
            .head(url)
            .send()
            .await?
            .headers()
            .get(CONTENT_LENGTH)
            .ok_or_else(|| anyhow!("Failed to get content length from '{}'", &url))?
            .to_str()?
            .parse::<u64>()?;

        let progress = ProgressBar::new(0);
        let draw_target = ProgressDrawTarget::stdout_with_hz(10);

        progress.set_draw_target(draw_target);
        progress.set_style(ProgressStyle::default_bar()
                .template("{spinner:.dim} {wide_bar:.cyan/blue} {percent:.bold}% {bytes}/{total_bytes} ({binary_bytes_per_sec:.bold.dim} elapsed: {elapsed:.bold.dim} eta: {eta:.bold.dim})")
                .tick_strings(&[
                    "◜",
                    "◠",
                    "◝",
                    "◞",
                    "◡",
                    "◟"
                ]),
            );

        progress.set_length(total_size);
        progress.set_message(title.to_owned());

        if !self.is_accept_ranges(url).await? {
            return Err(anyhow!(
                "Couldn't download file. Server doesn't support RANGE header!"
            ));
        }

        let chunk_size = total_size / threads;
        let mut start = 0;
        let mut ranges = vec![];
        while start < total_size {
            ranges.push((start, (start + chunk_size).min(total_size)));
            start += chunk_size + 1;
        }

        let mut promises: Vec<JoinHandle<Result<()>>> = vec![];
        let f = File::create(save_to.clone())?;
        let file = Arc::new(Mutex::new(f));

        for (_idx, (start, end)) in ranges.into_iter().enumerate() {
            let url = url.to_owned();
            let file = file.clone();

            let progress = progress.clone();

            promises.push(tokio::task::spawn(async move {
                let mut headers = HeaderMap::new();
                let range = format!("bytes={0}-{1}", start, end);
                headers.insert(RANGE, HeaderValue::from_str(&range).unwrap());

                let client = reqwest::Client::builder()
                    .default_headers(headers.clone())
                    .build()?;

                let response = client.get(url).send().await?;

                let mut stream = response.bytes_stream();
                let mut offset = start;

                while let Some(item) = stream.next().await {
                    let chunk = item?;
                    let mut f = file.lock().unwrap();
                    f.seek(std::io::SeekFrom::Start(offset))?;
                    f.write_all(&chunk)?;

                    offset += chunk.len() as u64;
                    progress.inc(chunk.len() as u64);
                }

                Ok(())
            }));
        }

        try_join_all(promises).await?;
        progress.finish_and_clear();

        Ok(())
    }
}
