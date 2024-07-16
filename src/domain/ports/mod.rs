use anyhow::Result;
use kube::api::DynamicObject;
use uuid::Uuid;

use super::{
    events::{PortCreatedEvent, PortCreatedEventProject},
    projects::Project,
};

pub mod create;

#[derive(Debug, Clone)]
pub struct Port {
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub data: String,
    pub created_by: String,
}
impl Port {
    pub fn new(project_id: &str, kind: &str, data: &str, created_by: &str) -> Self {
        let id = Uuid::new_v4().to_string();

        Self {
            id,
            project_id: project_id.into(),
            kind: kind.into(),
            data: data.into(),
            created_by: created_by.into(),
        }
    }
    pub fn to_event(&self, project: &Project) -> PortCreatedEvent {
        PortCreatedEvent {
            id: self.id.clone(),
            project: PortCreatedEventProject {
                id: self.project_id.clone(),
                namespace: project.namespace.clone(),
            },
            kind: self.kind.clone(),
            data: self.data.clone(),
            created_by: self.created_by.clone(),
        }
    }
}
impl From<PortCreatedEvent> for Port {
    fn from(value: PortCreatedEvent) -> Self {
        Self {
            id: value.id,
            project_id: value.project.id,
            kind: value.kind,
            data: value.data,
            created_by: value.created_by,
        }
    }
}

#[async_trait::async_trait]
pub trait PortCache: Send + Sync {
    async fn create(&self, port: &Port) -> Result<()>;
}

#[async_trait::async_trait]
pub trait PortCluster: Send + Sync {
    async fn create(&self, obj: &DynamicObject) -> Result<()>;
}
