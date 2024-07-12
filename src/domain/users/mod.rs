use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod create;

const AUTH_PROVIDER: &str = "auth0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub auth_provider: String,
    pub auth_provider_id: String,
}
impl User {
    pub fn new(email: String, auth_provider_id: String) -> Self {
        let id = Uuid::new_v4().to_string();

        Self {
            id,
            email,
            auth_provider: AUTH_PROVIDER.into(),
            auth_provider_id,
        }
    }
}

#[async_trait::async_trait]
pub trait UserCache: Send + Sync {
    async fn create(&self, user: &User) -> Result<()>;
    async fn get_by_auth_provider_id(&self, id: &str) -> Result<Option<User>>;
}

#[async_trait::async_trait]
pub trait AuthProvider: Send + Sync {
    async fn verify(&self, token: &str) -> Result<String>;
    async fn get_profile(&self, token: &str) -> Result<String>;
}
