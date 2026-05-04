//! Aiken docs fetcher with on-disk cache.
//!
//! Looks up canonical documentation pages by relative path under a configured
//! base URL (default: <https://aiken-lang.org>). Caches each response to a
//! sha256-keyed file under the cache directory so repeated lookups are
//! offline-friendly.
//!
//! v0 returns raw HTML/markdown. v1 may add HTML→markdown conversion and
//! symbol-aware extraction.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio::fs;
use tracing::debug;

use aiken_mcp_core::{CoreError, CoreResult, DocsFetcher};

#[derive(Debug, Clone)]
pub struct HttpDocsFetcher {
    base_url: String,
    cache_dir: PathBuf,
    client: reqwest::Client,
}

impl HttpDocsFetcher {
    pub fn new(base_url: impl Into<String>, cache_dir: PathBuf) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(concat!("aiken-mcp/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client init");
        Self {
            base_url: base_url.into(),
            cache_dir,
            client,
        }
    }

    pub fn default_for(cache_dir: PathBuf) -> Self {
        Self::new("https://aiken-lang.org", cache_dir)
    }

    fn cache_path(&self, key: &str) -> PathBuf {
        let digest = Sha256::digest(key.as_bytes());
        let hex = hex(&digest);
        self.cache_dir.join(format!("{hex}.cache"))
    }
}

#[async_trait]
impl DocsFetcher for HttpDocsFetcher {
    async fn fetch(&self, path: &str) -> CoreResult<String> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), path.trim_start_matches('/'));

        if let Ok(true) = fs::try_exists(&self.cache_dir).await {
            // OK
        } else {
            fs::create_dir_all(&self.cache_dir).await.ok();
        }

        let cache_file = self.cache_path(&url);
        if let Ok(bytes) = fs::read_to_string(&cache_file).await {
            debug!(path = %path, "docs cache hit");
            return Ok(bytes);
        }

        debug!(url = %url, "docs cache miss, fetching");
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CoreError::other(format!("docs fetch failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(CoreError::other(format!(
                "docs fetch returned {}: {}",
                resp.status(),
                url
            )));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| CoreError::other(format!("docs read failed: {e}")))?;

        if let Err(e) = fs::write(&cache_file, body.as_bytes()).await {
            debug!(error = %e, "failed to persist docs cache");
        }

        Ok(body)
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        write!(s, "{:02x}", b).unwrap();
    }
    s
}
