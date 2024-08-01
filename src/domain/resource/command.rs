use std::sync::Arc;

use anyhow::{bail, ensure, Result};
use chrono::Utc;
use tracing::info;
use uuid::Uuid;

use crate::domain::{
    auth::Credential,
    event::{EventDrivenBridge, ResourceCreated, ResourceDeleted},
    project::{cache::ProjectDrivenCache, Project},
    resource::ResourceStatus,
    PAGE_SIZE_DEFAULT, PAGE_SIZE_MAX,
};

use super::{cache::ResourceDrivenCache, Resource};

pub async fn fetch(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    cmd: FetchCmd,
) -> Result<Vec<Resource>> {
    assert_permission(project_cache.clone(), &cmd.credential, &cmd.project_id).await?;

    resource_cache
        .find(&cmd.project_id, &cmd.page, &cmd.page_size)
        .await
}

pub async fn create(
    project_cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: CreateCmd,
) -> Result<()> {
    assert_permission(project_cache.clone(), &cmd.credential, &cmd.project_id).await?;

    let Some(project) = project_cache.find_by_id(&cmd.project_id).await? else {
        bail!("project doesnt exist")
    };

    let evt = ResourceCreated {
        id: cmd.id,
        project_id: project.id,
        project_namespace: project.namespace,
        kind: cmd.kind.clone(),
        data: cmd.data,
        status: ResourceStatus::Active.to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(resource = cmd.kind, "new resource created");

    Ok(())
}

pub async fn delete(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: DeleteCmd,
) -> Result<()> {
    assert_permission(project_cache.clone(), &cmd.credential, &cmd.project_id).await?;

    let Some(project) = project_cache.find_by_id(&cmd.project_id).await? else {
        bail!("project doesnt exist")
    };

    let Some(resource) = resource_cache.find_by_id(&cmd.resource_id).await? else {
        bail!("resource doesnt exist")
    };

    assert_project_resource(&project, &resource)?;

    let evt = ResourceDeleted {
        id: cmd.resource_id,
        kind: resource.kind.clone(),
        status: ResourceStatus::Deleted.to_string(),
        project_id: project.id,
        project_namespace: project.namespace,
        deleted_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(resource = resource.kind, "resource deleted");

    Ok(())
}

async fn assert_permission(
    project_cache: Arc<dyn ProjectDrivenCache>,
    credential: &Credential,
    project_id: &str,
) -> Result<()> {
    match credential {
        Credential::Auth0(user_id) => {
            let result = project_cache
                .find_user_permission(user_id, project_id)
                .await?;
            ensure!(result.is_some(), "user doesnt have permission");
            Ok(())
        }
        Credential::ApiKey(secret_project_id) => {
            ensure!(
                project_id == secret_project_id,
                "secret doesnt have permission"
            );

            Ok(())
        }
    }
}
fn assert_project_resource(project: &Project, resource: &Resource) -> Result<()> {
    ensure!(project.id == resource.project_id);
    Ok(())
}

#[derive(Debug, Clone)]
pub struct FetchCmd {
    pub credential: Credential,
    pub project_id: String,
    pub page: u32,
    pub page_size: u32,
}
impl FetchCmd {
    pub fn new(
        credential: Credential,
        project_id: String,
        page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<Self> {
        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(PAGE_SIZE_DEFAULT);

        ensure!(
            page_size <= PAGE_SIZE_MAX,
            "page_size exceeded the limit of {PAGE_SIZE_MAX}"
        );

        Ok(Self {
            credential,
            project_id,
            page,
            page_size,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CreateCmd {
    pub credential: Credential,
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub data: String,
}
impl CreateCmd {
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

#[derive(Debug, Clone)]
pub struct DeleteCmd {
    pub credential: Credential,
    pub project_id: String,
    pub resource_id: String,
}

#[cfg(test)]
mod tests {
    use mockall::mock;
    use uuid::Uuid;

    use crate::domain::event::Event;
    use crate::domain::project::{Project, ProjectSecret, ProjectUser};

    use super::*;

    mock! {
        pub FakeProjectDrivenCache { }

        #[async_trait::async_trait]
        impl ProjectDrivenCache for FakeProjectDrivenCache {
            async fn find(&self, user_id: &str, page: &u32, page_size: &u32) -> Result<Vec<Project>>;
            async fn find_by_namespace(&self, namespace: &str) -> Result<Option<Project>>;
            async fn find_by_id(&self, id: &str) -> Result<Option<Project>>;
            async fn create(&self, project: &Project) -> Result<()>;
            async fn create_secret(&self, secret: &ProjectSecret) -> Result<()>;
            async fn find_secret_by_project_id(&self, project_id: &str) -> Result<Vec<ProjectSecret>>;
            async fn find_user_permission(&self,user_id: &str, project_id: &str) -> Result<Option<ProjectUser>>;
        }
    }

    mock! {
        pub FakeResourceDrivenCache { }

        #[async_trait::async_trait]
        impl ResourceDrivenCache for FakeResourceDrivenCache {
            async fn find(&self,project_id: &str,page: &u32,page_size: &u32) -> Result<Vec<Resource>>;
            async fn find_by_id(&self, id: &str) -> Result<Option<Resource>>;
            async fn create(&self, resource: &Resource) -> Result<()>;
            async fn delete(&self, id: &str) -> Result<()>;
        }
    }

    mock! {
        pub FakeEventDrivenBridge { }

        #[async_trait::async_trait]
        impl EventDrivenBridge for FakeEventDrivenBridge {
            async fn dispatch(&self, event: Event) -> Result<()>;
        }
    }

    impl Default for FetchCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                project_id: Uuid::new_v4().to_string(),
                page: 1,
                page_size: 12,
            }
        }
    }
    impl Default for CreateCmd {
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
    impl Default for DeleteCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                resource_id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
            }
        }
    }

    #[tokio::test]
    async fn it_should_fetch_project_resources() {
        let mut project_cache = MockFakeProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut resource_cache = MockFakeResourceDrivenCache::new();
        resource_cache
            .expect_find()
            .return_once(|_, _, _| Ok(vec![Resource::default()]));

        let cmd = FetchCmd::default();

        let result = fetch(Arc::new(project_cache), Arc::new(resource_cache), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_fetch_project_resources_when_user_doesnt_have_permission() {
        let mut project_cache = MockFakeProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let resource_cache = MockFakeResourceDrivenCache::new();

        let cmd = FetchCmd::default();

        let result = fetch(Arc::new(project_cache), Arc::new(resource_cache), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_fetch_project_resources_when_secret_doesnt_have_permission() {
        let project_cache = MockFakeProjectDrivenCache::new();
        let resource_cache = MockFakeResourceDrivenCache::new();

        let cmd = FetchCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = fetch(Arc::new(project_cache), Arc::new(resource_cache), cmd).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_create_resource() {
        let mut project_cache = MockFakeProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        project_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let mut event = MockFakeEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = CreateCmd::default();

        let result = create(Arc::new(project_cache), Arc::new(event), cmd).await;

        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_create_resource_when_project_doesnt_exist() {
        let mut project_cache = MockFakeProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        project_cache.expect_find_by_id().return_once(|_| Ok(None));

        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateCmd::default();

        let result = create(Arc::new(project_cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_resource_when_user_doesnt_have_permission() {
        let mut project_cache = MockFakeProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateCmd::default();

        let result = create(Arc::new(project_cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_resource_when_secret_doesnt_have_permission() {
        let project_cache = MockFakeProjectDrivenCache::new();

        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = create(Arc::new(project_cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_delete_resource() {
        let mut project_cache = MockFakeProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let project = Project::default();

        let project_cloned = project.clone();
        project_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(project_cloned)));

        let mut resource_cache = MockFakeResourceDrivenCache::new();
        resource_cache.expect_find_by_id().return_once(|_| {
            Ok(Some(Resource {
                project_id: project.id,
                ..Default::default()
            }))
        });

        let mut event = MockFakeEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = DeleteCmd::default();

        let result = delete(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(event),
            cmd,
        )
        .await;

        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_delete_resource_when_user_doesnt_have_permission() {
        let mut project_cache = MockFakeProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let resource_cache = MockFakeResourceDrivenCache::new();
        let event = MockFakeEventDrivenBridge::new();

        let cmd = DeleteCmd::default();

        let result = delete(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(event),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_delete_resource_when_secret_doesnt_have_permission() {
        let project_cache = MockFakeProjectDrivenCache::new();
        let resource_cache = MockFakeResourceDrivenCache::new();
        let event = MockFakeEventDrivenBridge::new();

        let cmd = DeleteCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = delete(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(event),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_delete_resource_when_resource_is_from_other_project() {
        let mut project_cache = MockFakeProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        project_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let mut resource_cache = MockFakeResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let event = MockFakeEventDrivenBridge::new();

        let cmd = DeleteCmd::default();

        let result = delete(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(event),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }
}
