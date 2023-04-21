use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;

use anyhow::Result;
use chrono::offset::Utc;
use chrono::Duration;

use crate::auth::token::{Token, TokenData};

pub trait TokenStorage {
    fn get(&self) -> Option<Token>;
    fn set(&self, data: &TokenData) -> Result<()>;
}

#[derive(Debug)]
pub struct JsonTokenStorage {
    filename: PathBuf,
}

impl JsonTokenStorage {
    pub(crate) fn new(filename: PathBuf) -> Self {
        Self { filename }
    }
}

impl TokenStorage for JsonTokenStorage {
    fn get(&self) -> Option<Token> {
        let file = File::open(&self.filename).ok()?;

        let reader = BufReader::new(file);
        let token_data: TokenData = serde_json::from_reader(reader).unwrap();

        if Utc::now() < token_data.updated_at + Duration::seconds(token_data.expires_in as i64) {
            return Some(Token::AccessToken(token_data.access_token));
        }

        if Utc::now() < token_data.updated_at + Duration::days(29) {
            return Some(Token::RefreshToken(token_data.refresh_token));
        }

        None
    }

    fn set(&self, token: &TokenData) -> Result<()> {
        log::debug!("saving token: {:?} to {:?}", token, self.filename);

        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.filename)
            .unwrap();
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, token)?;

        Ok(())
    }
}
