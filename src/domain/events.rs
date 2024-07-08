use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub project: String,
    pub kind: String,
    pub resource: Value,
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
