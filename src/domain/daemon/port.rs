use anyhow::Result;
use kube::{
    api::{ApiResource, DynamicObject, ObjectMeta},
    ResourceExt,
};
use std::sync::Arc;
use tracing::info;

use crate::domain::events::PortCreated;

pub async fn create_port(cluster: Arc<dyn PortCluster>, port: PortCreated) -> Result<()> {
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
        namespace: Some(port.project),
        ..Default::default()
    };
    obj.data = serde_json::from_str(&port.data)?;

    cluster.create(&obj).await?;

    //TODO: create event to update cache
    info!(port = obj.name_any(), "new port created");

    Ok(())
}

#[async_trait::async_trait]
pub trait PortCluster: Send + Sync {
    async fn create(&self, obj: &DynamicObject) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use mockall::mock;

    use crate::domain::management::port::Port;

    use super::*;

    mock! {
        pub FakePortCluster { }

        #[async_trait::async_trait]
        impl PortCluster for FakePortCluster {
            async fn create(&self, obj: &DynamicObject) -> Result<()>;
        }
    }

    #[tokio::test]
    async fn it_should_create_port() {
        let mut port_cluster = MockFakePortCluster::new();
        port_cluster.expect_create().return_once(|_| Ok(()));

        let port = Port::default();

        let result = create_port(Arc::new(port_cluster), port.into()).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
}
