use std::time::Duration;

use serde::Serialize;

use crate::config::Config;

const MAX_ATTEMPTS: u32 = 5;

pub fn build_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
}

/// Sends the payload as JSON. Retries with exponential backoff
/// (1s, 2s, 4s, 8s) if the server is unreachable or returns an error.
pub async fn send<T: Serialize>(
    client: &reqwest::Client,
    cfg: &Config,
    payload: &T,
) -> Result<(), String> {
    let mut delay = Duration::from_secs(1);
    let mut last_error = String::new();

    for attempt in 1..=MAX_ATTEMPTS {
        let result = client
            .post(&cfg.server_url)
            .bearer_auth(&cfg.auth_token)
            .json(payload)
            .send()
            .await;

        match result {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            Ok(resp) => last_error = format!("server returned {}", resp.status()),
            Err(e) => last_error = format!("request failed: {e}"),
        }

        if attempt < MAX_ATTEMPTS {
            eprintln!(
                "Send attempt {attempt}/{MAX_ATTEMPTS} failed ({last_error}), retrying in {}s",
                delay.as_secs()
            );
            tokio::time::sleep(delay).await;
            delay *= 2;
        }
    }

    Err(format!(
        "giving up after {MAX_ATTEMPTS} attempts: {last_error}"
    ))
}
