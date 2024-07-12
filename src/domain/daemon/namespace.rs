use anyhow::{bail, Result};
use k8s_openapi::api::core::v1::Namespace;
use kube::{api::ObjectMeta, ResourceExt};
use std::sync::Arc;
use tracing::info;

use crate::domain::events::ProjectCreated;

pub async fn create_namespace(
    cluster: Arc<dyn NamespaceCluster>,
    project: ProjectCreated,
) -> Result<()> {
    if cluster.find_by_name(&project.slug).await?.is_some() {
        bail!("namespace alread exist")
    }

    let ns: Namespace = project.into();
    cluster.create(&ns).await?;

    //TODO: create event to update cache
    info!(namespace = ns.name_any(), "new namespace created");

    Ok(())
}

impl From<ProjectCreated> for Namespace {
    fn from(value: ProjectCreated) -> Self {
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
pub trait NamespaceCluster: Send + Sync {
    async fn create(&self, namespace: &Namespace) -> Result<()>;
    async fn find_by_name(&self, name: &str) -> Result<Option<Namespace>>;
}

#[cfg(test)]
mod tests {
    use mockall::mock;

    use super::*;

    mock! {
        pub FakeNamespaceCluster { }

        #[async_trait::async_trait]
        impl NamespaceCluster for FakeNamespaceCluster {
            async fn create(&self, namespace: &Namespace) -> Result<()>;
            async fn find_by_name(&self, name: &str) -> Result<Option<Namespace>>;
        }
    }

    impl Default for ProjectCreated {
        fn default() -> Self {
            Self {
                name: "New Namespace".into(),
                slug: "sonic-vegas".into(),
            }
        }
    }

    #[tokio::test]
    async fn it_should_create_namespace() {
        let mut namespace_cluster = MockFakeNamespaceCluster::new();
        namespace_cluster.expect_create().return_once(|_| Ok(()));
        namespace_cluster
            .expect_find_by_name()
            .return_once(|_| Ok(None));

        let project_created = ProjectCreated::default();

        let result = create_namespace(Arc::new(namespace_cluster), project_created).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }

    #[tokio::test]
    async fn it_should_fail_when_namespace_exists() {
        let mut namespace_cluster = MockFakeNamespaceCluster::new();
        namespace_cluster.expect_create().return_once(|_| Ok(()));
        namespace_cluster
            .expect_find_by_name()
            .return_once(|_| Ok(Some(Namespace::default())));

        let project_created = ProjectCreated::default();

        let result = create_namespace(Arc::new(namespace_cluster), project_created).await;
        if result.is_ok() {
            unreachable!("Fail to validate when the namespace alread exists")
        }
    }
}
