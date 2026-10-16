use std::sync::Arc;

use k8s_openapi::api::core::v1::Namespace;
use kube::{api::ObjectMeta, ResourceExt};
use tracing::info;

use crate::domain::{
    event::{ProjectCreated, ProjectDeleted, ProjectExpired, ProvisionalProjectCreated},
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
    create_namespace(cluster, &evt.namespace).await
}

/// A provisional project gets its namespace exactly as an owned one does.
pub async fn apply_provisional_manifest(
    cluster: Arc<dyn ProjectDrivenCluster>,
    evt: ProvisionalProjectCreated,
) -> Result<()> {
    create_namespace(cluster, &evt.namespace).await
}

pub async fn delete_manifest(
    cluster: Arc<dyn ProjectDrivenCluster>,
    evt: ProjectDeleted,
) -> Result<()> {
    delete_namespace(cluster, &evt.namespace).await
}

/// An expired project loses its namespace, and every port in it, as a deleted one does.
pub async fn expire_manifest(
    cluster: Arc<dyn ProjectDrivenCluster>,
    evt: ProjectExpired,
) -> Result<()> {
    delete_namespace(cluster, &evt.namespace).await
}

async fn create_namespace(cluster: Arc<dyn ProjectDrivenCluster>, namespace: &str) -> Result<()> {
    let namespace = Namespace {
        metadata: ObjectMeta {
            name: Some(cluster_namespace(namespace)),
            ..Default::default()
        },
        ..Default::default()
    };
    cluster.create(&namespace).await?;

    //TODO: create event to update cache
    info!(namespace = namespace.name_any(), "new namespace created");

    Ok(())
}

async fn delete_namespace(cluster: Arc<dyn ProjectDrivenCluster>, namespace: &str) -> Result<()> {
    let namespace = Namespace {
        metadata: ObjectMeta {
            name: Some(cluster_namespace(namespace)),
            ..Default::default()
        },
        ..Default::default()
    };
    cluster.delete(&namespace).await?;

    //TODO: create event to update cache
    info!(namespace = namespace.name_any(), "namespace deleted");

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
