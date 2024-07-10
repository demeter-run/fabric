use anyhow::{Error, Result};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::domain::events::{Event, EventBridge, PortCreated};

use super::project::ProjectCache;

pub async fn create(
    project_cache: Arc<dyn ProjectCache>,
    event: Arc<dyn EventBridge>,
    port: Port,
) -> Result<()> {
    if project_cache.find_by_slug(&port.project).await?.is_none() {
        return Err(Error::msg("Invalid project"));
    }

    let port_event = Event::PortCreated(port.clone().into());

    event.dispatch(port_event).await?;
    info!(project = port.project, "new port requested");

    Ok(())
}

pub async fn create_cache(port_cache: Arc<dyn PortCache>, port: PortCreated) -> Result<()> {
    port_cache.create(&port.into()).await?;

    Ok(())
}

#[derive(Debug, Clone)]
pub struct Port {
    pub id: String,
    pub project: String,
    pub kind: String,
    pub data: String,
}
impl Port {
    pub fn new(project: &str, kind: &str, data: &str) -> Self {
        let id = Uuid::new_v4().to_string();

        Self {
            id,
            project: project.into(),
            kind: kind.into(),
            data: data.into(),
        }
    }
}
impl From<Port> for PortCreated {
    fn from(value: Port) -> Self {
        PortCreated {
            id: value.id,
            project: value.project,
            kind: value.kind,
            data: value.data,
        }
    }
}
impl From<PortCreated> for Port {
    fn from(value: PortCreated) -> Self {
        Port {
            id: value.id,
            project: value.project,
            kind: value.kind,
            data: value.data,
        }
    }
}

#[async_trait::async_trait]
pub trait PortCache: Send + Sync {
    async fn create(&self, port: &Port) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use mockall::mock;

    use super::*;
    use crate::domain::management::project::Project;

    mock! {
        pub FakeProjectCache { }

        #[async_trait::async_trait]
        impl ProjectCache for FakeProjectCache {
            async fn create(&self, project: &Project) -> Result<()>;
            async fn find_by_slug(&self, slug: &str) -> Result<Option<Project>>;
        }
    }

    mock! {
        pub FakePortCache { }

        #[async_trait::async_trait]
        impl PortCache for FakePortCache {
            async fn create(&self, port: &Port) -> Result<()>;
        }
    }

    mock! {
        pub FakeEventBridge { }

        #[async_trait::async_trait]
        impl EventBridge for FakeEventBridge {
            async fn dispatch(&self, event: Event) -> Result<()>;
        }
    }

    impl Default for Port {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project: "prj-test".into(),
                kind: "CardanoNode".into(),
                data: "{\"spec\":{\"operatorVersion\":\"1\",\"kupoVersion\":\"v1\",\"network\":\"mainnet\",\"pruneUtxo\":false,\"throughputTier\":\"0\"}}".into(),
            }
        }
    }

    #[tokio::test]
    async fn it_should_create_port() {
        let mut project_cache = MockFakeProjectCache::new();
        project_cache
            .expect_find_by_slug()
            .return_once(|_| Ok(Some(Project::default())));

        let mut event_bridge = MockFakeEventBridge::new();
        event_bridge.expect_dispatch().return_once(|_| Ok(()));

        let port = Port::default();

        let result = create(Arc::new(project_cache), Arc::new(event_bridge), port).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }

    #[tokio::test]
    async fn it_should_fail_when_project_not_exist() {
        let mut project_cache = MockFakeProjectCache::new();
        project_cache
            .expect_find_by_slug()
            .return_once(|_| Ok(None));

        let event_bridge = MockFakeEventBridge::new();

        let port = Port::default();

        let result = create(Arc::new(project_cache), Arc::new(event_bridge), port).await;
        if result.is_ok() {
            unreachable!("Fail to validate when the project doesnt exist")
        }
    }

    #[tokio::test]
    async fn it_should_create_port_cache() {
        let mut port_cache = MockFakePortCache::new();
        port_cache.expect_create().return_once(|_| Ok(()));

        let port = Port::default();

        let result = create_cache(Arc::new(port_cache), port.into()).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
}
