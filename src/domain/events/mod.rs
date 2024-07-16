use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCreatedEvent {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortCreatedEventProject {
    pub id: String,
    pub namespace: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortCreatedEvent {
    pub id: String,
    pub project: PortCreatedEventProject,
    pub kind: String,
    pub data: String,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::enum_variant_names)]
pub enum Event {
    ProjectCreated(ProjectCreatedEvent),
    PortCreated(PortCreatedEvent),
}
impl Event {
    pub fn key(&self) -> String {
        match self {
            Event::ProjectCreated(_) => "ProjectCreated".into(),
            Event::PortCreated(_) => "PortCreated".into(),
        }
    }
    pub fn from_key(key: &str, payload: &[u8]) -> Result<Self> {
        let event = match key {
            "ProjectCreated" => Self::ProjectCreated(serde_json::from_slice(payload)?),
            "PortCreated" => Self::PortCreated(serde_json::from_slice(payload)?),
            _ => bail!("Event key not implemented"),
        };
        Ok(event)
    }
}

#[async_trait::async_trait]
pub trait EventBridge: Send + Sync {
    async fn dispatch(&self, event: Event) -> Result<()>;
}
