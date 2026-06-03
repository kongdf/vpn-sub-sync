use anyhow::{Context, Result};
use std::time::Duration;

const MAX_RETRIES: u32 = 3;
const TIMEOUT_SECS: u64 = 20;

pub struct Fetcher {
    client: reqwest::Client,
}

impl Fetcher {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .user_agent("vpn-sub-sync/0.1.0")
            .build()?;
        Ok(Self { client })
    }

    pub async fn fetch_text(&self, url: &str) -> Result<String> {
        let mut last_err = None;

        for attempt in 1..=MAX_RETRIES {
            match self.try_fetch(url).await {
                Ok(body) => return Ok(body),
                Err(e) => {
                    eprintln!("  attempt {attempt}/{MAX_RETRIES} failed: {e}");
                    last_err = Some(e);
                    if attempt < MAX_RETRIES {
                        tokio::time::sleep(Duration::from_secs(attempt as u64)).await;
                    }
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("fetch failed")))
    }

    async fn try_fetch(&self, url: &str) -> Result<String> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("request {url}"))?;

        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("HTTP {status} for {url}");
        }

        resp.text()
            .await
            .with_context(|| format!("read body from {url}"))
    }
}
