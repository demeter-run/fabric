use std::sync::Arc;

use k8s_openapi::api::core::v1::Namespace;
use kube::{api::ObjectMeta, ResourceExt};
use tracing::info;

use crate::domain::{event::ProjectCreated, Result};

#[async_trait::async_trait]
pub trait ProjectDrivenCluster: Send + Sync {
    async fn create(&self, namespace: &Namespace) -> Result<()>;
}

pub async fn apply_manifest(
    cluster: Arc<dyn ProjectDrivenCluster>,
    evt: ProjectCreated,
) -> Result<()> {
    let namespace = Namespace {
        metadata: ObjectMeta {
            name: Some(evt.namespace),
            ..Default::default()
        },
        ..Default::default()
    };
    cluster.create(&namespace).await?;

    //TODO: create event to update cache
    info!(namespace = namespace.name_any(), "new namespace created");

    Ok(())
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::Namespace;
    use mockall::mock;

    use super::*;

    mock! {
        pub FakeProjectDrivenCluster { }

        #[async_trait::async_trait]
        impl ProjectDrivenCluster for FakeProjectDrivenCluster {
            async fn create(&self, namespace: &Namespace) -> Result<()>;
        }
    }

    #[tokio::test]
    async fn it_should_apply_manifest() {
        let mut cluster = MockFakeProjectDrivenCluster::new();
        cluster.expect_create().return_once(|_| Ok(()));

        let project = ProjectCreated::default();

        let result = apply_manifest(Arc::new(cluster), project).await;
        assert!(result.is_ok());
    }
}
