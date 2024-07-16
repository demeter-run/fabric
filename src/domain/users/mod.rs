use anyhow::Result;

pub mod auth;

#[async_trait::async_trait]
pub trait AuthProvider: Send + Sync {
    async fn verify(&self, token: &str) -> Result<String>;
}
