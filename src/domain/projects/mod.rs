use anyhow::Result;
use k8s_openapi::api::core::v1::Namespace;
use kube::api::ObjectMeta;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};

pub mod create;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub slug: String,
}
impl Project {
    pub fn new(name: String) -> Self {
        let slug: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();

        let slug = format!("prj-{}", slug.to_lowercase());

        Self { name, slug }
    }
}
impl From<Project> for Namespace {
    fn from(value: Project) -> Self {
        Namespace {
            metadata: ObjectMeta {
                name: Some(value.slug),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

#[async_trait::async_trait]
pub trait ProjectCache: Send + Sync {
    async fn create(&self, project: &Project) -> Result<()>;
    async fn find_by_slug(&self, slug: &str) -> Result<Option<Project>>;
}

#[async_trait::async_trait]
pub trait ProjectCluster: Send + Sync {
    async fn create(&self, namespace: &Namespace) -> Result<()>;
    async fn find_by_name(&self, name: &str) -> Result<Option<Namespace>>;
}
