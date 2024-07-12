use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCreated {
    pub name: String,
    pub slug: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCreated {
    pub id: String,
    pub email: String,
    pub auth_provider: String,
    pub auth_provider_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortCreated {
    pub id: String,
    pub project: String,
    pub kind: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::enum_variant_names)]
pub enum Event {
    ProjectCreated(ProjectCreated),
    UserCreated(UserCreated),
    PortCreated(PortCreated),
}

#[async_trait::async_trait]
pub trait EventBridge: Send + Sync {
    async fn dispatch(&self, event: Event) -> Result<()>;
}
