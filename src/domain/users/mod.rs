use anyhow::Result;

#[async_trait::async_trait]
pub trait AuthProvider: Send + Sync {
    fn verify(&self, token: &str) -> Result<String>;
}
