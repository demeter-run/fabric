use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
into_event!(ProjectCreated);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceCreated {
    pub id: String,
    pub project_id: String,
    pub project_namespace: String,
    pub kind: String,
    pub data: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
into_event!(ResourceCreated);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSecretCreated {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub phc: String,
    pub secret: Vec<u8>,
    pub created_at: DateTime<Utc>,
}
into_event!(ProjectSecretCreated);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::enum_variant_names)]
pub enum Event {
    ProjectCreated(ProjectCreated),
    ResourceCreated(ResourceCreated),
    ProjectSecretCreated(ProjectSecretCreated),
}
impl Event {
    pub fn key(&self) -> String {
        match self {
            Event::ProjectCreated(_) => "ProjectCreated".into(),
            Event::ResourceCreated(_) => "ResourceCreated".into(),
            Event::ProjectSecretCreated(_) => "ProjectSecretCreated".into(),
        }
    }
    pub fn from_key(key: &str, payload: &[u8]) -> Result<Self> {
        let event = match key {
            "ProjectCreated" => Self::ProjectCreated(serde_json::from_slice(payload)?),
            "ResourceCreated" => Self::ResourceCreated(serde_json::from_slice(payload)?),
            "ProjectSecretCreated" => Self::ProjectSecretCreated(serde_json::from_slice(payload)?),
            _ => bail!("Event key not implemented"),
        };
        Ok(event)
    }
}

#[async_trait::async_trait]
pub trait EventDrivenBridge: Send + Sync {
    async fn dispatch(&self, event: Event) -> Result<()>;
}
