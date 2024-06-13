use anyhow::{Error, Result};
use k8s_openapi::api::core::v1::Namespace;
use kube::{api::ObjectMeta, ResourceExt};
use std::sync::Arc;
use tracing::info;

use crate::domain::events::NamespaceCreation;

pub async fn create_namespace(
    cluster: Arc<dyn NamespaceCluster>,
    namespace: NamespaceCreation,
) -> Result<()> {
    if cluster.find_by_name(&namespace.name).await?.is_some() {
        return Err(Error::msg("namespace alread exist"));
    }

    let ns: Namespace = namespace.into();
    cluster.create(&ns).await?;

    //TODO: create event to update cache
    info!(namespace = ns.name_any(), "new namespace created");

    Ok(())
}

impl From<NamespaceCreation> for Namespace {
    fn from(value: NamespaceCreation) -> Self {
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

    impl Default for NamespaceCreation {
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

        let namespace_creation = NamespaceCreation::default();

        let result = create_namespace(Arc::new(namespace_cluster), namespace_creation).await;
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

        let namespace_creation = NamespaceCreation::default();

        let result = create_namespace(Arc::new(namespace_cluster), namespace_creation).await;
        if result.is_ok() {
            unreachable!("Fail to validate when the namespace alread exists")
        }
    }
}
