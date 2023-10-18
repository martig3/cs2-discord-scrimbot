use crate::commands::start::{ServerInfoResponse, StartMatch};
use crate::Config;
use base64::engine::general_purpose;
use base64::Engine;
use reqwest::{Client, Response, Result};
use std::time::Duration;

#[derive(Clone)]
pub struct DathostClient(Client);

impl DathostClient {
    pub async fn new(config: &Config) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::with_capacity(1);
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&{
                let username = config.dathost.username.clone();
                let password = config.dathost.password.clone();
                format!(
                    "Basic {}",
                    general_purpose::STANDARD
                        .encode(format!("{username}:{password}", password = password))
                )
            })
            .unwrap(),
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(60 * 10))
            .build()?;
        Ok(Self(client))
    }
    pub async fn get_server(&self, server_id: &String) -> Result<ServerInfoResponse> {
        Ok(self
            .0
            .get(&format!(
                "https://dathost.net/api/0.1/game-servers/{server_id}"
            ))
            .send()
            .await?
            .json()
            .await?)
    }

    pub async fn start_match(&self, body: &StartMatch) -> Result<Response> {
        self.0
            .post(&"https://dathost.net/api/0.1/cs2-matches".to_string())
            .json(&body)
            .send()
            .await
    }
}
