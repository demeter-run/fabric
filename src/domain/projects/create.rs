use anyhow::{bail, Result};
use kube::ResourceExt;
use std::sync::Arc;
use tracing::info;

use crate::domain::events::{Event, EventBridge, ProjectCreatedEvent};

use super::{Project, ProjectCache, ProjectCluster};

pub async fn create(
    cache: Arc<dyn ProjectCache>,
    event: Arc<dyn EventBridge>,
    project: Project,
) -> Result<()> {
    if cache.find_by_id(&project.namespace).await?.is_some() {
        bail!("invalid project namespace")
    }

    let project_event = Event::ProjectCreated(project.clone().into());

    event.dispatch(project_event).await?;
    info!(project = project.namespace, "new project created");

    Ok(())
}

pub async fn create_cache(
    cache: Arc<dyn ProjectCache>,
    project: ProjectCreatedEvent,
) -> Result<()> {
    cache.create(&project.into()).await?;

    Ok(())
}

pub async fn create_resource(
    cluster: Arc<dyn ProjectCluster>,
    project: ProjectCreatedEvent,
) -> Result<()> {
    if cluster.find_by_name(&project.namespace).await?.is_some() {
        bail!("namespace alread exist")
    }

    let namespace = project.into();
    cluster.create(&namespace).await?;

    //TODO: create event to update cache
    info!(namespace = namespace.name_any(), "new namespace created");

    Ok(())
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::Namespace;
    use mockall::mock;
    use uuid::Uuid;

    use super::*;
    use crate::domain::projects::ProjectUser;

    mock! {
        pub FakeProjectCache { }

        #[async_trait::async_trait]
        impl ProjectCache for FakeProjectCache {
            async fn create(&self, project: &Project) -> Result<()>;
            async fn find_by_id(&self, namespace: &str) -> Result<Option<Project>>;
            async fn find_user_permission(&self,user_id: &str,project_id: &str) -> Result<Option<ProjectUser>>;
        }
    }

    mock! {
        pub FakeEventBridge { }

        #[async_trait::async_trait]
        impl EventBridge for FakeEventBridge {
            async fn dispatch(&self, event: Event) -> Result<()>;
        }
    }

    mock! {
        pub FakeProjectCluster { }

        #[async_trait::async_trait]
        impl ProjectCluster for FakeProjectCluster {
            async fn create(&self, namespace: &Namespace) -> Result<()>;
            async fn find_by_name(&self, name: &str) -> Result<Option<Namespace>>;
        }
    }

    impl Default for Project {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                name: "New Project".into(),
                namespace: "sonic-vegas".into(),
                created_by: "user id".into(),
            }
        }
    }
    impl Default for ProjectUser {
        fn default() -> Self {
            Self {
                user_id: "user id".into(),
                project_id: Uuid::new_v4().to_string(),
            }
        }
    }

    #[tokio::test]
    async fn it_should_create_project() {
        let mut project_cache = MockFakeProjectCache::new();
        project_cache.expect_find_by_id().return_once(|_| Ok(None));

        let mut event_bridge = MockFakeEventBridge::new();
        event_bridge.expect_dispatch().return_once(|_| Ok(()));

        let project = Project::default();

        let result = create(Arc::new(project_cache), Arc::new(event_bridge), project).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }

    #[tokio::test]
    async fn it_should_fail_when_project_namespace_exist() {
        let mut project_cache = MockFakeProjectCache::new();
        project_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let event_bridge = MockFakeEventBridge::new();

        let project = Project::default();

        let result = create(Arc::new(project_cache), Arc::new(event_bridge), project).await;
        if result.is_ok() {
            unreachable!("Fail to validate when the namespace is duplicated")
        }
    }

    #[tokio::test]
    async fn it_should_create_project_cache() {
        let mut project_cache = MockFakeProjectCache::new();
        project_cache.expect_create().return_once(|_| Ok(()));

        let project = Project::default();

        let result = create_cache(Arc::new(project_cache), project.into()).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
    #[tokio::test]
    async fn it_should_create_project_resource() {
        let mut project_cluster = MockFakeProjectCluster::new();
        project_cluster.expect_create().return_once(|_| Ok(()));
        project_cluster
            .expect_find_by_name()
            .return_once(|_| Ok(None));

        let project = Project::default();

        let result = create_resource(Arc::new(project_cluster), project.into()).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }

    #[tokio::test]
    async fn it_should_fail_when_project_resource_exists() {
        let mut project_cluster = MockFakeProjectCluster::new();
        project_cluster.expect_create().return_once(|_| Ok(()));
        project_cluster
            .expect_find_by_name()
            .return_once(|_| Ok(Some(Namespace::default())));

        let project = Project::default();

        let result = create_resource(Arc::new(project_cluster), project.into()).await;
        if result.is_ok() {
            unreachable!("Fail to validate when the namespace alread exists")
        }
    }
}
