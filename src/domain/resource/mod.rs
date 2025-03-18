use std::{fmt::Display, str::FromStr};

use chrono::{DateTime, Utc};

use super::{
    error::Error,
    event::{ResourceCreated, ResourceUpdated},
};

pub mod cache;
pub mod cluster;
pub mod command;

pub struct Resource {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub kind: String,
    pub category: String,
    pub spec: String,
    pub annotations: Option<String>,
    pub status: ResourceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl TryFrom<ResourceCreated> for Resource {
    type Error = Error;

    fn try_from(value: ResourceCreated) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            project_id: value.project_id,
            name: value.name,
            kind: value.kind,
            category: value.category,
            spec: value.spec,
            annotations: None,
            status: value.status.parse()?,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

pub struct ResourceUpdate {
    pub id: String,
    pub spec_patch: String,
    pub updated_at: DateTime<Utc>,
}
impl TryFrom<ResourceUpdated> for ResourceUpdate {
    type Error = Error;

    fn try_from(value: ResourceUpdated) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            spec_patch: value.spec_patch,
            updated_at: value.updated_at,
        })
    }
}

#[derive(Debug, Clone)]
pub enum ResourceStatus {
    Active,
    Deleted,
}
impl FromStr for ResourceStatus {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "deleted" => Ok(Self::Deleted),
            _ => Err(Error::Unexpected("resource status not supported".into())),
        }
    }
}
impl Display for ResourceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ResourceProject {
    pub id: String,
    pub project_id: String,
    pub project_namespace: String,
    pub name: String,
    pub kind: String,
    pub category: String,
    pub spec: String,
    pub annotations: Option<String>,
    pub status: ResourceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::domain::utils;

    use super::*;

    impl Default for Resource {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                name: format!("cardanonode-{}", utils::get_random_salt()),
                kind: "CardanoNodePort".into(),
                spec: "{\"version\":\"stable\",\"network\":\"mainnet\",\"throughputTier\":\"1\"}"
                    .into(),
                category: "demeter-port".into(),
                annotations: None,
                status: ResourceStatus::Active,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        }
    }
}
