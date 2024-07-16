use anyhow::Result;

#[derive(Debug, Clone, Default)]
pub struct Credential {
    pub id: String,
}

#[async_trait::async_trait]
pub trait AuthProvider: Send + Sync {
    fn verify(&self, token: &str) -> Result<String>;
}
