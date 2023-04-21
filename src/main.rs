#![feature(result_option_inspect)]

use anyhow::Result;
use clap::Parser;
use cli_table::{print_stdout, WithTitle};

use crate::app::App;

mod api;
mod app;
mod auth;

mod parallel_downloader;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = app::Cli::parse();

    let mut logger = simple_logger::SimpleLogger::new().with_utc_timestamps();

    logger = match cli.verbose {
        1 => logger.with_level(log::LevelFilter::Info),
        2 => logger.with_level(log::LevelFilter::Debug),
        3 => logger.with_level(log::LevelFilter::Trace),
        _ => logger.with_level(log::LevelFilter::Error),
    };

    logger.init()?;

    let token_path = dirs::config_dir()
        .unwrap()
        .join("kinopub-auth-storage.json");

    log::debug!("auth storage path: {:?}", token_path);

    let storage = auth::storage::JsonTokenStorage::new(token_path);
    let mut config = api::Config::default();
    config.set_threads_count(cli.threads);

    let app_instance = App::new(&config, &storage);

    match &cli.command {
        app::Commands::Authenticate => {
            let current_user = app_instance.current_user().await?;

            println!(
                "Hello, {}!\nYou are successfully authenticated!",
                current_user.username
            );
        }
        app::Commands::Download {
            id,
            quality,
            season,
            episode,
        } => {
            app_instance
                .download(
                    id.to_owned(),
                    quality.to_owned(),
                    season.to_owned(),
                    episode.to_owned(),
                )
                .await?
        }
        app::Commands::Search { query } => {
            print_stdout(app_instance.search(query).await?.with_title())?;
        }
    }

    Ok(())
}
