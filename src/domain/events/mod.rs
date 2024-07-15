use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use super::{ports::Port, projects::Project, users::User};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::enum_variant_names)]
pub enum Event {
    ProjectCreated(Project),
    UserCreated(User),
    PortCreated(Port),
}
impl Event {
    pub fn key(&self) -> String {
        match self {
            Event::ProjectCreated(_) => "ProjectCreated".into(),
            Event::UserCreated(_) => "UserCreated".into(),
            Event::PortCreated(_) => "PortCreated".into(),
        }
    }
    pub fn from_key(key: &str, payload: &[u8]) -> Result<Self> {
        let event = match key {
            "ProjectCreated" => Self::ProjectCreated(serde_json::from_slice(payload)?),
            "UserCreated" => Self::UserCreated(serde_json::from_slice(payload)?),
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
