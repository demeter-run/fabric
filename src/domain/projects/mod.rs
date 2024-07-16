use anyhow::Result;
use k8s_openapi::api::core::v1::Namespace;
use kube::api::ObjectMeta;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod create;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub created_by: String,
}
impl Project {
    pub fn new(name: String, created_by: String) -> Self {
        let id = Uuid::new_v4().to_string();
        let namespace: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();

        let namespace = format!("prj-{}", namespace.to_lowercase());

        Self {
            id,
            name,
            namespace,
            created_by,
        }
    }
}
impl From<Project> for Namespace {
    fn from(value: Project) -> Self {
        Namespace {
            metadata: ObjectMeta {
                name: Some(value.namespace),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

#[async_trait::async_trait]
pub trait ProjectCache: Send + Sync {
    async fn create(&self, project: &Project) -> Result<()>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Project>>;
}

#[async_trait::async_trait]
pub trait ProjectCluster: Send + Sync {
    async fn create(&self, namespace: &Namespace) -> Result<()>;
    async fn find_by_name(&self, name: &str) -> Result<Option<Namespace>>;
}
