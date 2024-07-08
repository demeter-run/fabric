use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCreation {
    pub name: String,
    pub slug: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountCreation {
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortCreation {
    pub project: String,
    pub kind: String,
    pub resource: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::enum_variant_names)]
pub enum Event {
    ProjectCreation(ProjectCreation),
    AccountCreation(AccountCreation),
    PortCreation(PortCreation),
}

#[async_trait::async_trait]
pub trait EventBridge: Send + Sync {
    async fn dispatch(&self, event: Event) -> Result<()>;
}
