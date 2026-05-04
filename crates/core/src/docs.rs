use async_trait::async_trait;

use crate::error::CoreResult;

#[async_trait]
pub trait DocsFetcher: Send + Sync {
    async fn fetch(&self, path: &str) -> CoreResult<String>;
}
