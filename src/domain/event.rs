use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

macro_rules! into_event {
    ($name:ident) => {
        impl From<$name> for Event {
            fn from(value: $name) -> Self {
                Self::$name(value)
            }
        }
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCreated {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub owner: String,
}
into_event!(ProjectCreated);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceCreated {
    pub id: String,
    pub project_id: String,
    pub project_namespace: String,
    pub kind: String,
    pub data: String,
}
into_event!(ResourceCreated);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Event {
    ProjectCreated(ProjectCreated),
    ResourceCreated(ResourceCreated),
}
impl Event {
    pub fn key(&self) -> String {
        match self {
            Event::ProjectCreated(_) => "ProjectCreated".into(),
            Event::ResourceCreated(_) => "ResourceCreated".into(),
        }
    }
    pub fn from_key(key: &str, payload: &[u8]) -> Result<Self> {
        let event = match key {
            "ProjectCreated" => Self::ProjectCreated(serde_json::from_slice(payload)?),
            "ResourceCreated" => Self::ResourceCreated(serde_json::from_slice(payload)?),
            _ => bail!("Event key not implemented"),
        };
        Ok(event)
    }
}

#[async_trait::async_trait]
pub trait EventDrivenBridge: Send + Sync {
    async fn dispatch(&self, event: Event) -> Result<()>;
}
