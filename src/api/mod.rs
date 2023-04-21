use std::sync::{Arc, Mutex};

use anyhow::Result;
use chrono::serde::ts_seconds::deserialize as from_ts;
use chrono::{DateTime, Utc};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_struct_wrapper::deserialize_with_root;

use crate::utils::StringExt;

pub mod search;

#[derive(Debug, Clone)]
pub struct Config {
    pub api_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub threads: u64,
}

impl Config {
    pub fn set_threads_count(&mut self, threads: u64) {
        self.threads = threads;
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            client_id: "android".to_string(),
            client_secret: "rcaqh7wodackn9ll1uggvqkx2iib6umh".to_string(),
            api_url: "https://api.service-kp.com/".to_string(),
            threads: 4,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UserSubscription {
    pub days: f32,
    #[serde(deserialize_with = "from_ts")]
    pub end_time: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(remote = "Self")]
pub struct User {
    pub username: String,
    #[serde(deserialize_with = "from_ts")]
    pub reg_date: DateTime<Utc>,
    pub subscription: UserSubscription,
}
deserialize_with_root!("user": User);

#[derive(Debug, Deserialize)]
pub struct MovieFile {
    pub quality: String,
    pub codec: String,
    pub url: MovieUrl,
}

#[derive(Debug, Deserialize)]
pub struct MovieUrl {
    pub http: String,
}

impl ToString for MovieFile {
    fn to_string(&self) -> String {
        self.quality.to_owned()
    }
}

#[derive(Debug, Deserialize)]
pub struct Video {
    pub duration: u64,
    pub files: Vec<MovieFile>,
}

#[derive(Debug, Deserialize)]
#[serde(remote = "Self")]
pub struct Movie {
    pub title: String,
    pub year: u16,
    pub videos: Vec<Video>,
}
deserialize_with_root!("item": Movie);

#[derive(Debug, Deserialize)]
pub struct GeneralInfo {
    pub id: u64,
    pub title: String,
    pub year: u16,
    #[serde(rename = "plot")]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct Rating {
    #[serde(rename = "kinopoisk_rating")]
    pub kinopoisk: Option<f32>,
    #[serde(rename = "imdb_rating")]
    pub imdb: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct SeriesEpisode {
    pub id: u64,
    pub title: String,
    pub number: usize,
    pub files: Vec<MovieFile>,
}

#[derive(Debug, Deserialize)]
pub struct SeriesSeason {
    pub id: u64,
    pub title: String,
    pub number: usize,
    pub episodes: Vec<SeriesEpisode>,
}

#[derive(Debug, Deserialize)]
#[serde(remote = "Self", tag = "type")]
pub enum Item {
    #[serde(rename(deserialize = "movie"))]
    Movie {
        #[serde(flatten)]
        info: GeneralInfo,
        #[serde(flatten)]
        rating: Rating,
        videos: Vec<Video>,
    },
    #[serde(rename(deserialize = "serial"))]
    Series {
        #[serde(flatten)]
        info: GeneralInfo,
        #[serde(flatten)]
        rating: Rating,
        seasons: Vec<SeriesSeason>,
    },
    #[serde(rename(deserialize = "docuserial"))]
    DocSeries {
        #[serde(flatten)]
        info: GeneralInfo,
        #[serde(flatten)]
        rating: Rating,
        seasons: Vec<SeriesSeason>,
    },
    #[serde(rename(deserialize = "tvshow"))]
    TvShow {
        #[serde(flatten)]
        info: GeneralInfo,
        #[serde(flatten)]
        rating: Rating,
        seasons: Vec<SeriesSeason>,
    },
}
deserialize_with_root!("item": Item);

impl ToString for Movie {
    fn to_string(&self) -> String {
        self.title.to_owned()
    }
}

pub enum Api<R> {
    CurrentUser,
    ItemById(u64),
    Search(String),
    _Unreachable(std::convert::Infallible, std::marker::PhantomData<R>),
}

impl<R> ToString for Api<R> {
    fn to_string(&self) -> String {
        match self {
            Api::CurrentUser => "v1/user".to_string(),
            Api::ItemById(id) => format!("v1/items/{}", id),
            Api::Search(query) => {
                format!(
                    "v1/items/search?q={}&perpage=1000",
                    utf8_percent_encode(query, NON_ALPHANUMERIC)
                )
            }
            Api::_Unreachable(_, _) => unreachable!(),
        }
    }
}

pub struct ApiClient<'a> {
    config: &'a Config,
    client: Client,
    access_token: Arc<Mutex<String>>,
}

impl<'a> ApiClient<'a> {
    pub fn new(config: &'a Config) -> ApiClient {
        let client = reqwest::Client::new();
        ApiClient {
            config,
            client,
            access_token: Arc::new(Mutex::new("".to_string())),
        }
    }

    pub fn set_access_token(&self, access_token: &str) {
        let mut token = self.access_token.lock().unwrap();
        *token = access_token.to_owned();
    }

    pub async fn get<R: for<'de> Deserialize<'de>>(&self, api: Api<R>) -> Result<R> {
        self.get_decoded(&api.to_string()).await
    }

    async fn get_decoded<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = self.config.api_url.to_url()?.join(path)?;
        let mut req_builder = self.client.get(url);

        {
            let access_token = self.access_token.lock().unwrap();
            if !access_token.is_empty() {
                req_builder = req_builder.bearer_auth(access_token);
            }
        }

        Ok(req_builder.send().await?.json().await?)
    }
}
