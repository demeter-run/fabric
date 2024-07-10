use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCreated {
    pub name: String,
    pub slug: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountCreated {
    pub name: String,
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
    AccountCreated(AccountCreated),
    PortCreated(PortCreated),
}

#[async_trait::async_trait]
pub trait EventBridge: Send + Sync {
    async fn dispatch(&self, event: Event) -> Result<()>;
}
