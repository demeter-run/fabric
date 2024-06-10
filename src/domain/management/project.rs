use anyhow::{Error, Result};
use std::sync::Arc;

use super::events::{Event, EventBridge, NamespaceCreate};

pub async fn create(
    cache: Arc<dyn ProjectCache>,
    event: Arc<dyn EventBridge>,
    project: Project,
) -> Result<()> {
    if cache.find_by_slug(&project.slug).await?.is_some() {
        return Err(Error::msg("invalid project slug"));
    }

    cache.create(&project).await?;

    event.dispatch(project.into()).await?;

    Ok(())
}

#[derive(Debug, Clone)]
pub struct Project {
    pub name: String,
    pub description: String,
    pub slug: String,
}
impl From<Project> for Event {
    fn from(value: Project) -> Self {
        Event::NamespaceCreate(NamespaceCreate {
            slug: value.slug,
            name: value.name,
        })
    }
}

#[async_trait::async_trait]
pub trait ProjectCache {
    async fn create(&self, project: &Project) -> Result<()>;
    async fn find_by_slug(&self, slug: &str) -> Result<Option<Project>>;
}

#[cfg(test)]
mod tests {
    use mockall::mock;

    use super::*;

    mock! {
        pub FakeProjectCache { }

        #[async_trait::async_trait]
        impl ProjectCache for FakeProjectCache {
            async fn create(&self, project: &Project) -> Result<()>;
            async fn find_by_slug(&self, slug: &str) -> Result<Option<Project>>;
        }
    }

    mock! {
        pub FakeEventBridge { }

        #[async_trait::async_trait]
        impl EventBridge for FakeEventBridge {
            async fn dispatch(&self, event: Event) -> Result<()>;
        }
    }

    impl Default for Project {
        fn default() -> Self {
            Self {
                name: "New Project".into(),
                description: "Project to mock".into(),
                slug: "sonic-vegas".into(),
            }
        }
    }

    #[tokio::test]
    async fn it_should_create_project() {
        let mut project_cache = MockFakeProjectCache::new();
        project_cache
            .expect_find_by_slug()
            .return_once(|_| Ok(None));
        project_cache.expect_create().return_once(|_| Ok(()));

        let mut event_bridge = MockFakeEventBridge::new();
        event_bridge.expect_dispatch().return_once(|_| Ok(()));

        let project = Project::default();

        let result = create(Arc::new(project_cache), Arc::new(event_bridge), project).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }

    #[tokio::test]
    async fn it_should_fail_when_project_slug_exist() {
        let mut project_cache = MockFakeProjectCache::new();
        project_cache
            .expect_find_by_slug()
            .return_once(|_| Ok(Some(Project::default())));
        project_cache.expect_create().return_once(|_| Ok(()));

        let mut event_bridge = MockFakeEventBridge::new();
        event_bridge.expect_dispatch().return_once(|_| Ok(()));

        let project = Project::default();

        let result = create(Arc::new(project_cache), Arc::new(event_bridge), project).await;
        if result.is_ok() {
            unreachable!("Fail to validate when the slug is duplicated")
        }
    }
}
