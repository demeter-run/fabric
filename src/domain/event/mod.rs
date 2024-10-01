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
    pub billing_provider: String,
    pub billing_provider_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_subscription_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
into_event!(ProjectCreated);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectUpdated {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub updated_at: DateTime<Utc>,
}
into_event!(ProjectUpdated);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDeleted {
    pub id: String,
    pub namespace: String,
    pub deleted_at: DateTime<Utc>,
}
into_event!(ProjectDeleted);

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
pub struct ProjectSecretDeleted {
    pub id: String,
    pub deleted_by: String,
    pub deleted_at: DateTime<Utc>,
}
into_event!(ProjectSecretDeleted);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectUserInviteCreated {
    pub id: String,
    pub project_id: String,
    pub email: String,
    pub role: String,
    pub code: String,
    pub expires_in: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
into_event!(ProjectUserInviteCreated);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectUserInviteAccepted {
    pub id: String,
    pub project_id: String,
    pub user_id: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}
into_event!(ProjectUserInviteAccepted);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectUserDeleted {
    pub id: String,
    pub project_id: String,
    pub user_id: String,
    pub role: String,
    pub deleted_by: String,
    pub deleted_at: DateTime<Utc>,
}
into_event!(ProjectUserDeleted);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceCreated {
    pub id: String,
    pub project_id: String,
    pub project_namespace: String,
    pub name: String,
    pub kind: String,
    pub spec: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
into_event!(ResourceCreated);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUpdated {
    pub id: String,
    pub project_id: String,
    pub project_namespace: String,
    pub name: String,
    pub kind: String,
    pub spec_patch: String,
    pub updated_at: DateTime<Utc>,
}
into_event!(ResourceUpdated);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDeleted {
    pub id: String,
    pub project_id: String,
    pub project_namespace: String,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub deleted_at: DateTime<Utc>,
}
into_event!(ResourceDeleted);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageUnitCreated {
    pub resource_name: String,
    pub tier: String,
    pub units: i64,
    pub interval: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageCreated {
    pub id: String,
    pub cluster_id: String,
    pub project_namespace: String,
    pub usages: Vec<UsageUnitCreated>,
    pub created_at: DateTime<Utc>,
}
into_event!(UsageCreated);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Event {
    ProjectCreated(ProjectCreated),
    ProjectUpdated(ProjectUpdated),
    ProjectDeleted(ProjectDeleted),
    ProjectSecretCreated(ProjectSecretCreated),
    ProjectSecretDeleted(ProjectSecretDeleted),
    ProjectUserInviteCreated(ProjectUserInviteCreated),
    ProjectUserInviteAccepted(ProjectUserInviteAccepted),
    ProjectUserDeleted(ProjectUserDeleted),
    ResourceCreated(ResourceCreated),
    ResourceUpdated(ResourceUpdated),
    ResourceDeleted(ResourceDeleted),
    UsageCreated(UsageCreated),
}
impl Event {
    pub fn key(&self) -> String {
        match self {
            Event::ProjectCreated(_) => "ProjectCreated".into(),
            Event::ProjectUpdated(_) => "ProjectUpdated".into(),
            Event::ProjectDeleted(_) => "ProjectDeleted".into(),
            Event::ProjectSecretCreated(_) => "ProjectSecretCreated".into(),
            Event::ProjectSecretDeleted(_) => "ProjectSecretDeleted".into(),
            Event::ProjectUserInviteCreated(_) => "ProjectUserInviteCreated".into(),
            Event::ProjectUserInviteAccepted(_) => "ProjectUserInviteAccepted".into(),
            Event::ProjectUserDeleted(_) => "ProjectUserDeleted".into(),
            Event::ResourceCreated(_) => "ResourceCreated".into(),
            Event::ResourceUpdated(_) => "ResourceUpdated".into(),
            Event::ResourceDeleted(_) => "ResourceDeleted".into(),
            Event::UsageCreated(_) => "UsageCreated".into(),
        }
    }
    pub fn from_key(key: &str, payload: &[u8]) -> Result<Self> {
        match key {
            "ProjectCreated" => Ok(Self::ProjectCreated(serde_json::from_slice(payload)?)),
            "ProjectUpdated" => Ok(Self::ProjectUpdated(serde_json::from_slice(payload)?)),
            "ProjectDeleted" => Ok(Self::ProjectDeleted(serde_json::from_slice(payload)?)),
            "ProjectSecretCreated" => {
                Ok(Self::ProjectSecretCreated(serde_json::from_slice(payload)?))
            }
            "ProjectSecretDeleted" => {
                Ok(Self::ProjectSecretDeleted(serde_json::from_slice(payload)?))
            }
            "ProjectUserInviteCreated" => Ok(Self::ProjectUserInviteCreated(
                serde_json::from_slice(payload)?,
            )),
            "ProjectUserInviteAccepted" => Ok(Self::ProjectUserInviteAccepted(
                serde_json::from_slice(payload)?,
            )),
            "ProjectUserDeleted" => Ok(Self::ProjectUserDeleted(serde_json::from_slice(payload)?)),
            "ResourceCreated" => Ok(Self::ResourceCreated(serde_json::from_slice(payload)?)),
            "ResourceUpdated" => Ok(Self::ResourceUpdated(serde_json::from_slice(payload)?)),
            "ResourceDeleted" => Ok(Self::ResourceDeleted(serde_json::from_slice(payload)?)),
            "UsageCreated" => Ok(Self::UsageCreated(serde_json::from_slice(payload)?)),
            _ => Err(Error::Unexpected(format!(
                "Event key '{}' not implemented",
                key
            ))),
        }
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait EventDrivenBridge: Send + Sync {
    async fn dispatch(&self, event: Event) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use uuid::Uuid;

    use crate::domain::{
        project::{ProjectStatus, ProjectUserRole},
        resource::ResourceStatus,
        tests::{PHC, SECRET},
        utils::get_random_salt,
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
                billing_provider: "stripe".into(),
                billing_provider_id: "stripe id".into(),
                billing_subscription_id: None,
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
    impl Default for ProjectSecretDeleted {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                deleted_by: Uuid::new_v4().to_string(),
                deleted_at: Utc::now(),
            }
        }
    }
    impl Default for ProjectUserInviteCreated {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                email: "p@txpipe.io".into(),
                code: "123".into(),
                role: ProjectUserRole::Owner.to_string(),
                expires_in: Utc::now() + Duration::from_secs(15 * 60),
                created_at: Utc::now(),
            }
        }
    }
    impl Default for ProjectUserInviteAccepted {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                user_id: "user id".into(),
                role: ProjectUserRole::Owner.to_string(),
                created_at: Utc::now(),
            }
        }
    }
    impl Default for ResourceCreated {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                project_namespace: "test".into(),
                name: format!("cardanonode-{}", get_random_salt()),
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
                project_id: Uuid::new_v4().to_string(),
                project_namespace: "test".into(),
                name: format!("cardanonode-{}", get_random_salt()),
                kind: "CardanoNodePort".into(),
                status: ResourceStatus::Deleted.to_string(),
                deleted_at: Utc::now(),
            }
        }
    }
    impl Default for UsageCreated {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                cluster_id: Uuid::new_v4().to_string(),
                project_namespace: "test".into(),
                usages: vec![UsageUnitCreated {
                    resource_name: format!("cardanonode-{}", get_random_salt()),
                    units: 120,
                    tier: "0".into(),
                    interval: 10,
                }],
                created_at: Utc::now(),
            }
        }
    }
}
