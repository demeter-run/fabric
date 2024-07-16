use anyhow::{bail, Result};
use kube::{
    api::{ApiResource, DynamicObject, ObjectMeta},
    ResourceExt,
};
use std::sync::Arc;
use tracing::info;

use crate::domain::{
    events::{Event, EventBridge},
    projects::ProjectCache,
};

use super::{Port, PortCache, PortCluster};

pub async fn create(
    project_cache: Arc<dyn ProjectCache>,
    event: Arc<dyn EventBridge>,
    port: Port,
) -> Result<()> {
    if project_cache.find_by_id(&port.project_id).await?.is_none() {
        bail!("Invalid project")
    }

    let port_event = Event::PortCreated(port.clone());

    event.dispatch(port_event).await?;
    info!(project = port.project_id, "new port requested");

    Ok(())
}

pub async fn create_cache(port_cache: Arc<dyn PortCache>, port: Port) -> Result<()> {
    port_cache.create(&port).await?;

    Ok(())
}

pub async fn create_resource(cluster: Arc<dyn PortCluster>, port: Port) -> Result<()> {
    let api = ApiResource {
        kind: port.kind.clone(),
        group: "demeter.run".into(),
        version: "v1alpha1".into(),
        plural: format!("{}s", port.kind.clone().to_lowercase()),
        api_version: "demeter.run/v1alpha1".into(),
    };

    let mut obj = DynamicObject::new(&port.id, &api);
    obj.metadata = ObjectMeta {
        name: Some(port.id),
        namespace: Some(port.project_id),
        ..Default::default()
    };
    obj.data = serde_json::from_str(&port.data)?;

    cluster.create(&obj).await?;

    //TODO: create event to update cache
    info!(port = obj.name_any(), "new port created");

    Ok(())
}

#[cfg(test)]
mod tests {
    use mockall::mock;
    use uuid::Uuid;

    use super::*;
    use crate::domain::projects::Project;

    mock! {
        pub FakeProjectCache { }

        #[async_trait::async_trait]
        impl ProjectCache for FakeProjectCache {
            async fn create(&self, project: &Project) -> Result<()>;
            async fn find_by_id(&self, id: &str) -> Result<Option<Project>>;
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

    mock! {
        pub FakePortCluster { }

        #[async_trait::async_trait]
        impl PortCluster for FakePortCluster {
            async fn create(&self, obj: &DynamicObject) -> Result<()>;
        }
    }

    impl Default for Port {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                kind: "CardanoNode".into(),
                data: "{\"spec\":{\"operatorVersion\":\"1\",\"kupoVersion\":\"v1\",\"network\":\"mainnet\",\"pruneUtxo\":false,\"throughputTier\":\"0\"}}".into(),
            }
        }
    }

    #[tokio::test]
    async fn it_should_create_port() {
        let mut project_cache = MockFakeProjectCache::new();
        project_cache
            .expect_find_by_id()
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
        project_cache.expect_find_by_id().return_once(|_| Ok(None));

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

        let result = create_cache(Arc::new(port_cache), port).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }

    #[tokio::test]
    async fn it_should_create_resource() {
        let mut port_cluster = MockFakePortCluster::new();
        port_cluster.expect_create().return_once(|_| Ok(()));

        let port = Port::default();

        let result = create_resource(Arc::new(port_cluster), port).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
}
