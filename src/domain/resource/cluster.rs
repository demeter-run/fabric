use std::sync::Arc;

use kube::{
    api::{ApiResource, DynamicObject, ObjectMeta},
    ResourceExt,
};
use tracing::info;

use crate::domain::{
    event::{ResourceCreated, ResourceDeleted},
    Result,
};

#[async_trait::async_trait]
pub trait ResourceDrivenCluster: Send + Sync {
    async fn create(&self, obj: &DynamicObject) -> Result<()>;
    async fn delete(&self, obj: &DynamicObject) -> Result<()>;
}
pub async fn apply_manifest(
    cluster: Arc<dyn ResourceDrivenCluster>,
    evt: ResourceCreated,
) -> Result<()> {
    let api = build_api_resource(&evt.kind);

    let mut obj = DynamicObject::new(&evt.id, &api);
    obj.metadata = ObjectMeta {
        name: Some(evt.id),
        namespace: Some(evt.project_namespace),
        ..Default::default()
    };

    let spec = serde_json::from_str(&evt.spec)?;
    obj.data = serde_json::json!({ "spec": serde_json::Value::Object(spec) });

    cluster.create(&obj).await?;

    //TODO: create event to update cache
    info!(resource = obj.name_any(), "new resource created");

    Ok(())
}
pub async fn delete_manifest(
    cluster: Arc<dyn ResourceDrivenCluster>,
    evt: ResourceDeleted,
) -> Result<()> {
    let api = build_api_resource(&evt.kind);

    let mut obj = DynamicObject::new(&evt.id, &api);
    obj.metadata = ObjectMeta {
        name: Some(evt.id),
        namespace: Some(evt.project_namespace),
        ..Default::default()
    };

    cluster.delete(&obj).await?;

    info!(resource = obj.name_any(), "resource deleted");

    Ok(())
}

fn build_api_resource(kind: &str) -> ApiResource {
    ApiResource {
        kind: kind.into(),
        group: "demeter.run".into(),
        version: "v1alpha1".into(),
        plural: format!("{}s", kind.to_string().to_lowercase()),
        api_version: "demeter.run/v1alpha1".into(),
    }
}

#[cfg(test)]
mod tests {
    use mockall::mock;

    use super::*;

    mock! {
        pub FakeResourceDrivenCluster { }

        #[async_trait::async_trait]
        impl ResourceDrivenCluster for FakeResourceDrivenCluster {
            async fn create(&self, obj: &DynamicObject) -> Result<()>;
            async fn delete(&self, obj: &DynamicObject) -> Result<()>;
        }
    }

    #[tokio::test]
    async fn it_should_apply_manifest() {
        let mut cluster = MockFakeResourceDrivenCluster::new();
        cluster.expect_create().return_once(|_| Ok(()));

        let evt = ResourceCreated::default();

        let result = apply_manifest(Arc::new(cluster), evt).await;
        assert!(result.is_ok());
    }
}
