use anyhow::{bail, Result};
use kube::{
    api::{ApiResource, DynamicObject, ObjectMeta},
    ResourceExt,
};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use super::{
    auth::Credential,
    event::{EventDrivenBridge, ResourceCreated},
    project::ProjectDrivenCache,
};

pub async fn create(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: CreateResourceCmd,
) -> Result<()> {
    // TODO validate credential

    let Some(project) = cache.find_by_id(&cmd.project_id).await? else {
        bail!("project doesnt exist")
    };

    let evt = ResourceCreated {
        id: cmd.id,
        project_id: project.id,
        project_namespace: project.namespace,
        kind: cmd.kind.clone(),
        data: cmd.data,
    };

    event.dispatch(evt.into()).await?;
    info!(resource = cmd.kind, "new resource created");

    Ok(())
}

pub async fn create_cache(cache: Arc<dyn ResourceDrivenCache>, evt: ResourceCreated) -> Result<()> {
    cache.create(&evt.into()).await?;

    Ok(())
}

pub async fn apply_manifest(
    cluster: Arc<dyn ResourceDrivenCluster>,
    evt: ResourceCreated,
) -> Result<()> {
    let api = ApiResource {
        kind: evt.kind.clone(),
        group: "demeter.run".into(),
        version: "v1alpha1".into(),
        plural: format!("{}s", evt.kind.clone().to_lowercase()),
        api_version: "demeter.run/v1alpha1".into(),
    };

    let mut obj = DynamicObject::new(&evt.id, &api);
    obj.metadata = ObjectMeta {
        name: Some(evt.id),
        namespace: Some(evt.project_namespace),
        ..Default::default()
    };
    obj.data = serde_json::from_str(&evt.data)?;

    cluster.create(&obj).await?;

    //TODO: create event to update cache
    info!(resource = obj.name_any(), "new resource created");

    Ok(())
}

#[derive(Debug, Clone)]
pub struct CreateResourceCmd {
    pub credential: Credential,
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub data: String,
}
impl CreateResourceCmd {
    pub fn new(credential: Credential, project_id: String, kind: String, data: String) -> Self {
        let id = Uuid::new_v4().to_string();

        Self {
            credential,
            id,
            project_id,
            kind,
            data,
        }
    }
}

pub struct ResourceCache {
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub data: String,
}
impl From<ResourceCreated> for ResourceCache {
    fn from(value: ResourceCreated) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            kind: value.kind,
            data: value.data,
        }
    }
}

#[async_trait::async_trait]
pub trait ResourceDrivenCache: Send + Sync {
    async fn create(&self, resource: &ResourceCache) -> Result<()>;
}

#[async_trait::async_trait]
pub trait ResourceDrivenCluster: Send + Sync {
    async fn create(&self, obj: &DynamicObject) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use mockall::mock;
    use uuid::Uuid;

    use crate::domain::event::Event;
    use crate::domain::project::{ProjectCache, ProjectSecretCache};

    use super::*;

    mock! {
        pub FakeProjectDrivenCache { }

        #[async_trait::async_trait]
        impl ProjectDrivenCache for FakeProjectDrivenCache {
            async fn find_by_namespace(&self, namespace: &str) -> Result<Option<ProjectCache>>;
            async fn find_by_id(&self, id: &str) -> Result<Option<ProjectCache>>;
            async fn create(&self, project: &ProjectCache) -> Result<()>;
            async fn create_secret(&self, secret: &ProjectSecretCache) -> Result<()>;
            async fn find_secret_by_project_id(&self, project_id: &str) -> Result<Vec<ProjectSecretCache>>;
        }
    }

    mock! {
        pub FakeResourceDrivenCache { }

        #[async_trait::async_trait]
        impl ResourceDrivenCache for FakeResourceDrivenCache {
            async fn create(&self, resource: &ResourceCache) -> Result<()>;
        }
    }

    mock! {
        pub FakeEventDrivenBridge { }

        #[async_trait::async_trait]
        impl EventDrivenBridge for FakeEventDrivenBridge {
            async fn dispatch(&self, event: Event) -> Result<()>;
        }
    }

    mock! {
        pub FakeResourceDrivenCluster { }

        #[async_trait::async_trait]
        impl ResourceDrivenCluster for FakeResourceDrivenCluster {
            async fn create(&self, obj: &DynamicObject) -> Result<()>;
        }
    }

    impl Default for CreateResourceCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                kind: "CardanoNode".into(),
                data: "{\"spec\":{\"operatorVersion\":\"1\",\"kupoVersion\":\"v1\",\"network\":\"mainnet\",\"pruneUtxo\":false,\"throughputTier\":\"0\"}}".into(),
            }
        }
    }
    impl Default for ResourceCreated {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                project_namespace: "prj-test".into(),
                kind: "CardanoNode".into(),
                data: "{\"spec\":{\"operatorVersion\":\"1\",\"kupoVersion\":\"v1\",\"network\":\"mainnet\",\"pruneUtxo\":false,\"throughputTier\":\"0\"}}".into(),
            }
        }
    }

    #[tokio::test]
    async fn it_should_create_resource() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(ProjectCache::default())));

        let mut event = MockFakeEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = CreateResourceCmd::default();

        let result = create(Arc::new(cache), Arc::new(event), cmd).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
    #[tokio::test]
    async fn it_should_fail_when_project_doesnt_exist() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache.expect_find_by_id().return_once(|_| Ok(None));

        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateResourceCmd::default();

        let result = create(Arc::new(cache), Arc::new(event), cmd).await;
        if result.is_ok() {
            unreachable!("Fail to validate when the project doesnt exist")
        }
    }

    #[tokio::test]
    async fn it_should_create_resource_cache() {
        let mut cache = MockFakeResourceDrivenCache::new();
        cache.expect_create().return_once(|_| Ok(()));

        let evt = ResourceCreated::default();

        let result = create_cache(Arc::new(cache), evt).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }

    #[tokio::test]
    async fn it_should_apply_manifest() {
        let mut cluster = MockFakeResourceDrivenCluster::new();
        cluster.expect_create().return_once(|_| Ok(()));

        let evt = ResourceCreated::default();

        let result = apply_manifest(Arc::new(cluster), evt).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
}
