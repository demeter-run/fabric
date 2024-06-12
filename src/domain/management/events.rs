use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Event {
    NamespaceCreation(Namespace),
}

#[async_trait::async_trait]
pub trait EventBridge: Send + Sync {
    async fn dispatch(&self, event: Event) -> Result<()>;
}
