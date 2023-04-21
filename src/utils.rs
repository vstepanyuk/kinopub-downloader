use anyhow::Result;
use thiserror::Error;
use url::Url;

use crate::api::Item;

#[derive(Debug, Error)]
pub enum UtilsError {
    #[error("Season {0} is not found")]
    SeasonNotFound(usize),
    #[error("Episode {0} is not found")]
    EpisodeNotFound(usize),
}

pub trait StringExt {
    fn map_not_empty<T, F>(self, transform: F) -> T
    where
        F: Fn(Self) -> T,
        Self: Into<T>;

    fn to_url(&self) -> Result<Url>;
}

impl StringExt for String {
    fn map_not_empty<T, F>(self, transform: F) -> T
    where
        F: Fn(Self) -> T,
        Self: Into<T>,
    {
        if self.is_empty() {
            return self.into();
        }

        transform(self)
    }

    fn to_url(&self) -> Result<Url> {
        Ok(Url::parse(self)?)
    }
}

pub struct Utils;

impl Utils {
    pub fn generate_filename(
        item: &Item,
        quality: &str,
        season: Option<usize>,
        episode: Option<usize>,
    ) -> Result<String> {
        let info = match item {
            Item::Movie { info, .. } => info,
            Item::Series { info, .. } => info,
            Item::DocSeries { info, .. } => info,
            Item::TvShow { info, .. } => info,
        };

        let title = if info.title.contains('/') {
            let (rus_title, eng_title) = info.title.split_once('/').unwrap();
            format!("{} ({})", rus_title.trim(), eng_title.trim())
        } else {
            info.title.to_owned()
        };

        match item {
            Item::TvShow { seasons, .. }
            | Item::Series { seasons, .. }
            | Item::DocSeries { seasons, .. } => {
                let season_number = season.unwrap_or(1);
                let episode_number = episode.unwrap_or(1);

                let season = seasons
                    .iter()
                    .find(|s| s.number == season_number)
                    .ok_or(UtilsError::SeasonNotFound(season_number))?;

                let seasons_width = seasons.len().to_string().len();

                let episode = season
                    .episodes
                    .iter()
                    .find(|e| e.number == episode_number)
                    .ok_or(UtilsError::EpisodeNotFound(episode_number))?;

                let episode_width = season.episodes.len().to_string().len();

                let season_title = format!(
                    "Season: {:0width$}{}",
                    season_number,
                    season
                        .title
                        .clone()
                        .map_not_empty(|title| format!(" {}", title)),
                    width = seasons_width
                );

                let episode_title = format!(
                    "Episode: {:0width$}{}",
                    episode_number,
                    episode
                        .title
                        .clone()
                        .map_not_empty(|title| format!(" {}", title)),
                    width = episode_width
                );

                return Ok(format!(
                    "{0} [{2}, {3}] [{1}].mp4",
                    title, quality, season_title, episode_title
                ));
            }
            _ => {}
        }

        Ok(format!("{0} [{1}].mp4", title, quality))
    }
}
