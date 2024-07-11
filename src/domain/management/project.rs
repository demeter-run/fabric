use anyhow::{bail, Result};
use rand::{distributions::Alphanumeric, Rng};
use std::sync::Arc;
use tracing::info;

use crate::domain::events::{Event, EventBridge, ProjectCreated};

pub async fn create(
    cache: Arc<dyn ProjectCache>,
    event: Arc<dyn EventBridge>,
    project: Project,
) -> Result<()> {
    if cache.find_by_slug(&project.slug).await?.is_some() {
        bail!("invalid project slug")
    }

    let namespace = Event::ProjectCreated(project.clone().into());

    event.dispatch(namespace).await?;
    info!(project = project.slug, "new project created");

    Ok(())
}

pub async fn create_cache(cache: Arc<dyn ProjectCache>, project: ProjectCreated) -> Result<()> {
    cache.create(&project.into()).await?;

    Ok(())
}

#[derive(Debug, Clone)]
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
impl From<ProjectCreated> for Project {
    fn from(value: ProjectCreated) -> Self {
        Self {
            name: value.name,
            slug: value.slug,
        }
    }
}
impl From<Project> for ProjectCreated {
    fn from(value: Project) -> Self {
        ProjectCreated {
            name: value.name,
            slug: value.slug,
        }
    }
}

#[async_trait::async_trait]
pub trait ProjectCache: Send + Sync {
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

        let event_bridge = MockFakeEventBridge::new();

        let project = Project::default();

        let result = create(Arc::new(project_cache), Arc::new(event_bridge), project).await;
        if result.is_ok() {
            unreachable!("Fail to validate when the slug is duplicated")
        }
    }

    #[tokio::test]
    async fn it_should_create_project_cache() {
        let mut project_cache = MockFakeProjectCache::new();
        project_cache.expect_create().return_once(|_| Ok(()));

        let project = Project::default();
        let project_event = ProjectCreated {
            name: project.name,
            slug: project.slug,
        };

        let result = create_cache(Arc::new(project_cache), project_event).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
}
