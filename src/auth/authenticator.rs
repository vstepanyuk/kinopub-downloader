use std::fmt::Debug;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use thiserror::Error;
use tokio::time::{sleep, timeout};
use url::Url;

use crate::api::Config;
use crate::auth::storage::TokenStorage;
use crate::auth::token::{Token, TokenData};

#[derive(Debug, Deserialize)]
struct CodeResponse {
    code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Deserialize)]
struct AuthorizationResponseError {
    // status: u16,
    error: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    // token_type: Option<String>,
}

impl From<TokenResponse> for TokenData {
    fn from(r: TokenResponse) -> Self {
        TokenData {
            refresh_token: r.refresh_token,
            access_token: r.access_token,
            expires_in: r.expires_in,
            updated_at: Utc::now(),
        }
    }
}

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Authorization error: {0}")]
    Authorization(String),
}

#[derive(Debug)]
pub struct Authenticator<'a, Storage>
where
    Storage: TokenStorage,
{
    config: &'a Config,
    storage: &'a Storage,
    client: reqwest::Client,
}

impl<'a, Storage> Authenticator<'a, Storage>
where
    Storage: TokenStorage,
{
    pub fn new(config: &'a Config, storage: &'a Storage) -> Authenticator<'a, Storage> {
        let client = reqwest::Client::new();

        Self {
            config,
            client,
            storage,
        }
    }

    async fn get_device_code(&self) -> Result<CodeResponse> {
        let auth_url = self.build_url("/oauth2/device")?;

        let params = [
            ("grant_type", "device_code"),
            ("client_id", &self.config.client_id),
            ("client_secret", &self.config.client_secret),
        ];

        let result: CodeResponse = self
            .client
            .post(auth_url)
            .form(&params)
            .send()
            .await?
            .json()
            .await?;

        println!(
            "Please enter '{}' at {}",
            result.user_code, result.verification_uri
        );

        Ok(result)
    }

    pub async fn authenticate(&self) -> Result<String> {
        if let Some(token) = self.storage.get() {
            match token {
                Token::AccessToken(access_token) => return Ok(access_token),
                Token::RefreshToken(refresh_token) => {
                    let result = self.refresh_token(&refresh_token).await;

                    if let Some(token) = result {
                        let token_data = token.into();
                        self.storage.set(&token_data)?;

                        return Ok(token_data.access_token);
                    }
                }
            }
        }

        let response = self.get_device_code().await?;

        let token = timeout(
            Duration::from_secs(response.expires_in),
            self.wait_for_device_authorization(&response.code, response.interval),
        )
        .await??;

        let token_data = token.into();
        self.storage.set(&token_data)?;

        self.notify(&token_data.access_token).await?;

        Ok(token_data.access_token)
    }

    async fn refresh_token(&self, refresh_token: &str) -> Option<TokenResponse> {
        let url = self.build_url("/oauth2/device").ok()?;

        let params = [
            ("grant_type", "refresh_token"),
            ("client_id", &self.config.client_id),
            ("client_secret", &self.config.client_secret),
            ("refresh_token", refresh_token),
        ];

        self.client
            .post(url)
            .form(&params)
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()
    }

    async fn wait_for_device_authorization(
        &self,
        code: &str,
        interval: u64,
    ) -> Result<TokenResponse> {
        loop {
            let url = self.build_url("/oauth2/device")?;
            let params = [
                ("grant_type", "device_token"),
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
                ("code", code),
            ];

            let res = self.client.post(url).form(&params).send().await?;
            if res.status().is_success() {
                let token: TokenResponse = res.json().await?;

                return Ok(token);
            }

            let result: AuthorizationResponseError = res.json().await?;
            if result.error != "authorization_pending" {
                return Err(AuthError::Authorization(result.error).into());
            }

            sleep(Duration::from_secs(interval)).await;
        }
    }

    async fn notify(&self, access_token: &str) -> anyhow::Result<()> {
        let url = self.build_url("/v1/device/notify")?;

        let mid = match machine_uid::get() {
            Ok(result) => result,
            Err(err) => panic!("{}", err),
        };

        let software = format!("{} {}", sys_info::os_type()?, sys_info::os_release()?);

        let params = [
            ("title", "CLI Downloader"),
            ("hardware", mid.as_str()),
            ("software", software.as_str()),
        ];

        self.client
            .post(url)
            .bearer_auth(access_token)
            .form(&params)
            .send()
            .await?;

        Ok(())
    }

    fn build_url(&self, path: &str) -> Result<Url> {
        Ok(Url::parse(&self.config.api_url)?.join(path)?)
    }
}
