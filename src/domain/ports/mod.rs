use anyhow::Result;
use kube::api::DynamicObject;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod create;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Port {
    pub id: String,
    pub project: String,
    pub kind: String,
    pub data: String,
}
impl Port {
    pub fn new(project: &str, kind: &str, data: &str) -> Self {
        let id = Uuid::new_v4().to_string();

        Self {
            id,
            project: project.into(),
            kind: kind.into(),
            data: data.into(),
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
