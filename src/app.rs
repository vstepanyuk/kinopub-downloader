use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::Deserialize;

use auth::Authenticator;

use crate::api::search::{SearchResult, SearchResultItem};
use crate::api::{Api, ApiClient, Config, Item, User};
use crate::auth::storage::TokenStorage;
use crate::utils::Utils;
use crate::{auth, parallel_downloader::Downloader};

#[derive(Parser)]
#[clap(author = "Vitali Stsepaniuk <contact@vitaliy.dev>", version, about)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,

    #[clap(short, long, parse(from_occurrences))]
    pub verbose: usize,

    #[clap(short, long, default_value_t = 4)]
    pub threads: u64,
}

#[derive(Subcommand)]
pub enum Commands {
    Download {
        #[clap(short = 'i', long = "id", help = "Item ID")]
        id: u64,
        #[clap(short = 'q', long, help = "Quality (2160p, 1080p, 720p, 480p)")]
        quality: Option<String>,
        #[clap(
            short = 's',
            long,
            help = "Season # (only for TV series), default: all"
        )]
        season: Option<usize>,
        #[clap(
            short = 'e',
            long,
            help = "Episode # (only for TV series), default: all"
        )]
        episode: Option<usize>,
    },
    Authenticate,
    Search {
        #[clap(short = 'q', long, help = "Search query")]
        query: String,
    },
}

pub struct App<'a, Storage>
where
    Storage: TokenStorage,
{
    auth: Authenticator<'a, Storage>,
    api_client: ApiClient<'a>,
    config: &'a Config,
}

impl<'a, Storage> App<'a, Storage>
where
    Storage: TokenStorage,
{
    pub fn new(config: &'a Config, storage: &'a Storage) -> App<'a, Storage> {
        let auth = Authenticator::new(config, storage);
        let api_client = ApiClient::new(config);
        Self {
            auth,
            api_client,
            config,
        }
    }

    pub async fn current_user(&self) -> Result<User> {
        self.request(Api::CurrentUser).await
    }

    pub async fn search(&self, query: &str) -> Result<Vec<SearchResultItem>> {
        self.request(Api::Search(query.to_string()))
            .await
            .map(|r: SearchResult| r.items)
    }

    pub async fn download(
        &self,
        id: u64,
        quality: Option<String>,
        season: Option<usize>,
        episode: Option<usize>,
    ) -> Result<()> {
        let item: &Item = &self.request(Api::ItemById(id)).await?;
        let quality = quality.unwrap_or_else(|| "720p".to_owned());

        match item {
            Item::Movie { videos, .. } => {
                if let Some(file) = videos
                    .first()
                    .and_then(|v| v.files.iter().find(|f| f.quality == quality))
                {
                    let filename = Utils::generate_filename(item, &quality, season, episode)?;

                    return self
                        .download_single_file(&filename, &file.url.http, &filename)
                        .await;
                }

                eprintln!("File with {} quality is not found.", quality);
                std::process::exit(1);
            }
            Item::Series { seasons, .. }
            | Item::TvShow { seasons, .. }
            | Item::DocSeries { seasons, .. } => {
                for s in seasons {
                    if season.is_some() && season.unwrap() != s.number {
                        continue;
                    }

                    for e in s.episodes.iter() {
                        if episode.is_some() && episode.unwrap() != e.number {
                            continue;
                        }

                        if let Some(file) = e.files.iter().find(|f| f.quality == quality) {
                            let filename = Utils::generate_filename(
                                item,
                                &quality,
                                Some(s.number),
                                Some(e.number),
                            )?;

                            self.download_single_file(&filename, &file.url.http, &filename)
                                .await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn download_single_file(&self, title: &str, url: &str, filename: &str) -> Result<()> {
        let save_to = std::env::current_dir().unwrap().join(filename);

        Downloader::default()
            .download_to(url, title, save_to, self.config.threads)
            .await
    }

    async fn request<T: for<'de> Deserialize<'de>>(&self, api: Api<T>) -> Result<T> {
        let access_token = self.auth.authenticate().await?;
        self.api_client.set_access_token(&access_token);
        self.api_client.get(api).await
    }
}
