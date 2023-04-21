use chrono::serde::ts_seconds::{deserialize as from_ts, serialize as to_ts};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenData {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,

    #[serde(serialize_with = "to_ts", deserialize_with = "from_ts")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub enum Token {
    AccessToken(String),
    RefreshToken(String),
}

impl TokenData {}
