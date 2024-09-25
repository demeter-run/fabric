use std::sync::Arc;

use k8s_openapi::api::core::v1::Namespace;
use kube::{api::ObjectMeta, ResourceExt};
use tracing::info;

use crate::domain::{
    event::{ProjectCreated, ProjectDeleted},
    utils::cluster_namespace,
    Result,
};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ProjectDrivenCluster: Send + Sync {
    async fn create(&self, namespace: &Namespace) -> Result<()>;
    async fn delete(&self, namespace: &Namespace) -> Result<()>;
}

pub async fn apply_manifest(
    cluster: Arc<dyn ProjectDrivenCluster>,
    evt: ProjectCreated,
) -> Result<()> {
    let namespace = Namespace {
        metadata: ObjectMeta {
            name: Some(cluster_namespace(&evt.namespace)),
            ..Default::default()
        },
        ..Default::default()
    };
    cluster.create(&namespace).await?;

    //TODO: create event to update cache
    info!(namespace = namespace.name_any(), "new namespace created");

    Ok(())
}

pub async fn delete_manifest(
    cluster: Arc<dyn ProjectDrivenCluster>,
    evt: ProjectDeleted,
) -> Result<()> {
    let namespace = Namespace {
        metadata: ObjectMeta {
            name: Some(cluster_namespace(&evt.namespace)),
            ..Default::default()
        },
        ..Default::default()
    };
    cluster.delete(&namespace).await?;

    //TODO: create event to update cache
    info!(namespace = namespace.name_any(), "new namespace created");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn it_should_apply_manifest() {
        let mut cluster = MockProjectDrivenCluster::new();
        cluster.expect_create().return_once(|_| Ok(()));

        let project = ProjectCreated::default();

        let result = apply_manifest(Arc::new(cluster), project).await;
        assert!(result.is_ok());
    }
}
