use anyhow::Result;
use kube::{
    api::{ApiResource, DynamicObject, ObjectMeta},
    ResourceExt,
};
use rand::{distributions::Alphanumeric, Rng};
use std::sync::Arc;
use tracing::info;

use crate::domain::events::PortCreated;

pub async fn create_port(cluster: Arc<dyn PortCluster>, port: PortCreated) -> Result<()> {
    let slug: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();
    let name = format!("{}-{}", port.kind.clone().to_lowercase(), slug);

    let api = ApiResource {
        kind: port.kind.clone(),
        group: "demeter.run".into(),
        version: "v1alpha1".into(),
        plural: format!("{}s", port.kind.clone().to_lowercase()),
        api_version: "demeter.run/v1alpha1".into(),
    };

    let mut obj = DynamicObject::new(&name, &api);
    obj.metadata = ObjectMeta {
        name: Some(name),
        namespace: Some(port.project),
        ..Default::default()
    };
    obj.data = port.resource;

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

    impl Default for PortCreated {
        fn default() -> Self {
            Self {
                kind: "KupoPort".into(),
                project: "prj-xxxxxxx".into(),
                resource:  "{\"spec\":{\"operatorVersion\":\"1\",\"kupoVersion\":\"v1\",\"network\":\"mainnet\",\"pruneUtxo\":false,\"throughputTier\":\"0\"}}".into()
            }
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
