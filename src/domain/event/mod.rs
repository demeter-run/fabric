use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::Result;

use super::error::Error;

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
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
into_event!(ProjectCreated);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectUpdated {
    pub id: String,
    pub name: Option<String>,
    pub status: Option<String>,
    pub updated_at: DateTime<Utc>,
}
into_event!(ProjectUpdated);

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
pub struct ResourceCreated {
    pub id: String,
    pub project_id: String,
    pub project_namespace: String,
    pub kind: String,
    pub spec: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
into_event!(ResourceCreated);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDeleted {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub project_id: String,
    pub project_namespace: String,
    pub deleted_at: DateTime<Utc>,
}
into_event!(ResourceDeleted);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::enum_variant_names)]
pub enum Event {
    ProjectCreated(ProjectCreated),
    ProjectUpdated(ProjectUpdated),
    ProjectSecretCreated(ProjectSecretCreated),
    ResourceCreated(ResourceCreated),
    ResourceDeleted(ResourceDeleted),
}
impl Event {
    pub fn key(&self) -> String {
        match self {
            Event::ProjectCreated(_) => "ProjectCreated".into(),
            Event::ProjectUpdated(_) => "ProjectUpdated".into(),
            Event::ProjectSecretCreated(_) => "ProjectSecretCreated".into(),
            Event::ResourceCreated(_) => "ResourceCreated".into(),
            Event::ResourceDeleted(_) => "ResourceDeleted".into(),
        }
    }
    pub fn from_key(key: &str, payload: &[u8]) -> Result<Self> {
        match key {
            "ProjectCreated" => Ok(Self::ProjectCreated(serde_json::from_slice(payload)?)),
            "ProjectUpdated" => Ok(Self::ProjectUpdated(serde_json::from_slice(payload)?)),
            "ProjectSecretCreated" => {
                Ok(Self::ProjectSecretCreated(serde_json::from_slice(payload)?))
            }
            "ResourceCreated" => Ok(Self::ResourceCreated(serde_json::from_slice(payload)?)),
            "ResourceDeleted" => Ok(Self::ResourceDeleted(serde_json::from_slice(payload)?)),
            _ => Err(Error::Unexpected(format!(
                "Event key '{}' not implemented",
                key
            ))),
        }
    }
}

#[async_trait::async_trait]
pub trait EventDrivenBridge: Send + Sync {
    async fn dispatch(&self, event: Event) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::domain::{
        project::ProjectStatus,
        resource::ResourceStatus,
        tests::{PHC, SECRET},
    };

    use super::*;

    impl Default for ProjectCreated {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                name: "New Project".into(),
                namespace: "sonic-vegas".into(),
                owner: "user id".into(),
                status: ProjectStatus::Active.to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        }
    }
    impl Default for ProjectSecretCreated {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                name: "Key 1".into(),
                phc: PHC.into(),
                secret: SECRET.as_bytes().to_vec(),
                created_at: Utc::now(),
            }
        }
    }
    impl Default for ResourceCreated {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                project_namespace: "prj-test".into(),
                kind: "CardanoNodePort".into(),
                spec: "{\"version\":\"stable\",\"network\":\"mainnet\",\"throughputTier\":\"1\"}"
                    .into(),
                status: ResourceStatus::Active.to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        }
    }
    impl Default for ResourceDeleted {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                kind: "CardanoNodePort".into(),
                status: ResourceStatus::Deleted.to_string(),
                project_id: Uuid::new_v4().to_string(),
                project_namespace: "prj-test".into(),
                deleted_at: Utc::now(),
            }
        }
    }
}
