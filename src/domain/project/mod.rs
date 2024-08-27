use std::{fmt::Display, str::FromStr};

use chrono::{DateTime, Utc};

use super::{
    error::Error,
    event::{ProjectCreated, ProjectPaymentCreated, ProjectSecretCreated, ProjectUpdated},
};

pub mod cache;
pub mod cluster;
pub mod command;

#[derive(Debug, Clone)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub owner: String,
    pub status: ProjectStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl TryFrom<ProjectCreated> for Project {
    type Error = Error;

    fn try_from(value: ProjectCreated) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            namespace: value.namespace,
            name: value.name,
            owner: value.owner,
            status: value.status.parse()?,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ProjectUpdate {
    pub id: String,
    pub name: Option<String>,
    pub status: Option<ProjectStatus>,
    pub updated_at: DateTime<Utc>,
}
impl TryFrom<ProjectUpdated> for ProjectUpdate {
    type Error = Error;

    fn try_from(value: ProjectUpdated) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            name: value.name,
            status: match value.status {
                Some(status) => Some(status.parse()?),
                None => None,
            },
            updated_at: value.updated_at,
        })
    }
}

#[derive(Debug, Clone)]
pub enum ProjectStatus {
    Active,
    Deleted,
    PaymentMethodFailed,
}
impl FromStr for ProjectStatus {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "active" => Ok(ProjectStatus::Active),
            "dcu-consumed" => Ok(ProjectStatus::Active),
            "pm-failed" => Ok(ProjectStatus::PaymentMethodFailed),
            "deleted" => Ok(ProjectStatus::Deleted),
            _ => Err(Error::Unexpected(format!(
                "project status not supported: {}",
                s
            ))),
        }
    }
}
impl Display for ProjectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectStatus::Active => write!(f, "active"),
            ProjectStatus::Deleted => write!(f, "deleted"),
            ProjectStatus::PaymentMethodFailed => write!(f, "pm-failed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectSecret {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub phc: String,
    pub secret: Vec<u8>,
    pub created_at: DateTime<Utc>,
}
impl From<ProjectSecretCreated> for ProjectSecret {
    fn from(value: ProjectSecretCreated) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            name: value.name,
            phc: value.phc,
            secret: value.secret,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectPayment {
    pub id: String,
    pub project_id: String,
    pub provider: String,
    pub provider_id: String,
    pub subscription_id: Option<String>,
    pub created_at: DateTime<Utc>,
}
impl From<ProjectPaymentCreated> for ProjectPayment {
    fn from(value: ProjectPaymentCreated) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            provider: value.provider,
            provider_id: value.provider_id,
            subscription_id: value.subscription_id,
            created_at: value.created_at,
        }
    }
}

#[allow(dead_code)]
pub struct ProjectUser {
    pub user_id: String,
    pub project_id: String,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::domain::tests::{PHC, SECRET};

    use super::*;

    impl Default for Project {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                name: "New Project".into(),
                namespace: "sonic-vegas".into(),
                owner: "user id".into(),
                status: ProjectStatus::Active,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        }
    }
    impl Default for ProjectSecret {
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
    impl Default for ProjectUser {
        fn default() -> Self {
            Self {
                user_id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                created_at: Utc::now(),
            }
        }
    }
    impl Default for ProjectPayment {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                provider: "stripe".into(),
                provider_id: "stripe id".into(),
                subscription_id: None,
                created_at: Utc::now(),
            }
        }
    }
}
