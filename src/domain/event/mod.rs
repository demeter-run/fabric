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
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
into_event!(ProjectCreated);

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
    pub data: String,
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
    ProjectSecretCreated(ProjectSecretCreated),
    ResourceCreated(ResourceCreated),
    ResourceDeleted(ResourceDeleted),
}
impl Event {
    pub fn key(&self) -> String {
        match self {
            Event::ProjectCreated(_) => "ProjectCreated".into(),
            Event::ProjectSecretCreated(_) => "ProjectSecretCreated".into(),
            Event::ResourceCreated(_) => "ResourceCreated".into(),
            Event::ResourceDeleted(_) => "ResourceDeleted".into(),
        }
    }
    pub fn from_key(key: &str, payload: &[u8]) -> Result<Self> {
        let event = match key {
            "ProjectCreated" => Self::ProjectCreated(serde_json::from_slice(payload)?),
            "ProjectSecretCreated" => Self::ProjectSecretCreated(serde_json::from_slice(payload)?),
            "ResourceCreated" => Self::ResourceCreated(serde_json::from_slice(payload)?),
            "ResourceDeleted" => Self::ResourceDeleted(serde_json::from_slice(payload)?),
            _ => bail!("Event key not implemented"),
        };
        Ok(event)
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
                kind: "CardanoNode".into(),
                data: "{\"spec\":{\"operatorVersion\":\"1\",\"kupoVersion\":\"v1\",\"network\":\"mainnet\",\"pruneUtxo\":false,\"throughputTier\":\"0\"}}".into(),
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
                kind: "CardanoNode".into(),
                status: ResourceStatus::Deleted.to_string(),
                project_id: Uuid::new_v4().to_string(),
                project_namespace: "prj-test".into(),
                deleted_at: Utc::now(),
            }
        }
    }
}
