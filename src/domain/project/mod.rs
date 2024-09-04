use std::{fmt::Display, str::FromStr};

use chrono::{DateTime, Utc};

use super::{
    error::Error,
    event::{
        ProjectCreated, ProjectSecretCreated, ProjectUpdated, ProjectUserInviteAccepted,
        ProjectUserInviteCreated,
    },
    Result,
};

pub mod cache;
pub mod cluster;
pub mod command;

#[async_trait::async_trait]
pub trait StripeDriven: Send + Sync {
    async fn create_customer(&self, name: &str, email: &str) -> Result<String>;
}

#[async_trait::async_trait]
pub trait ProjectEmailDriven: Send + Sync {
    async fn send_invite(
        &self,
        project_name: &str,
        email: &str,
        code: &str,
        expires_in: &DateTime<Utc>,
    ) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub owner: String,
    pub status: ProjectStatus,
    pub billing_provider: String,
    pub billing_provider_id: String,
    pub billing_subscription_id: Option<String>,
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
            billing_provider: value.billing_provider,
            billing_provider_id: value.billing_provider_id,
            billing_subscription_id: value.billing_subscription_id,
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
pub struct ProjectUserInvite {
    pub id: String,
    pub project_id: String,
    pub email: String,
    pub code: String,
    pub role: ProjectUserRole,
    pub status: ProjectUserInviteStatus,
    pub expires_in: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl TryFrom<ProjectUserInviteCreated> for ProjectUserInvite {
    type Error = Error;

    fn try_from(value: ProjectUserInviteCreated) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            project_id: value.project_id,
            email: value.email,
            role: value.role.parse()?,
            code: value.code,
            status: ProjectUserInviteStatus::Sent,
            expires_in: value.expires_in,
            created_at: value.created_at,
            updated_at: value.created_at,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectUserInviteStatus {
    Sent,
    Accepted,
}
impl FromStr for ProjectUserInviteStatus {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "sent" => Ok(ProjectUserInviteStatus::Sent),
            "accepted" => Ok(ProjectUserInviteStatus::Accepted),
            _ => Err(Error::Unexpected(format!(
                "project user invite status not supported: {}",
                s
            ))),
        }
    }
}
impl Display for ProjectUserInviteStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectUserInviteStatus::Sent => write!(f, "sent"),
            ProjectUserInviteStatus::Accepted => write!(f, "accepted"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectUser {
    pub user_id: String,
    pub project_id: String,
    pub role: ProjectUserRole,
    pub created_at: DateTime<Utc>,
}
impl TryFrom<ProjectUserInviteAccepted> for ProjectUser {
    type Error = Error;

    fn try_from(value: ProjectUserInviteAccepted) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            project_id: value.project_id,
            user_id: value.user_id,
            role: value.role.parse()?,
            created_at: value.created_at,
        })
    }
}

#[derive(Debug, Clone)]
pub enum ProjectUserRole {
    Owner,
    Member,
}
impl FromStr for ProjectUserRole {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "owner" => Ok(ProjectUserRole::Owner),
            "member" => Ok(ProjectUserRole::Member),
            _ => Err(Error::Unexpected(format!(
                "project user role not supported: {}",
                s
            ))),
        }
    }
}
impl Display for ProjectUserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectUserRole::Owner => write!(f, "owner"),
            ProjectUserRole::Member => write!(f, "member"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

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
                billing_provider: "stripe".into(),
                billing_provider_id: "stripe id".into(),
                billing_subscription_id: None,
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
                role: ProjectUserRole::Owner,
                created_at: Utc::now(),
            }
        }
    }
    impl Default for ProjectUserInvite {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                email: "p@txpipe.io".into(),
                role: ProjectUserRole::Owner,
                code: "123".into(),
                status: ProjectUserInviteStatus::Sent,
                expires_in: Utc::now() + Duration::from_secs(15 * 60),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        }
    }
}
