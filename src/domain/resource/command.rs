use std::sync::Arc;

use argon2::{password_hash::SaltString, Argon2};
use base64::{prelude::BASE64_STANDARD_NO_PAD, Engine};
use bech32::{Bech32m, Hrp};
use chrono::Utc;
use rand::rngs::OsRng;
use tracing::info;
use uuid::Uuid;

use crate::domain::{
    auth::{assert_project_permission, Credential},
    error::Error,
    event::{EventDrivenBridge, ResourceCreated, ResourceDeleted},
    metadata::{KnownField, MetadataDriven},
    project::{cache::ProjectDrivenCache, Project},
    resource::{ResourceStatus, ResourceUpdated},
    utils::get_schema_from_crd,
    Result, PAGE_SIZE_DEFAULT, PAGE_SIZE_MAX,
};

use super::{cache::ResourceDrivenCache, Resource};

pub async fn fetch(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    cmd: FetchCmd,
) -> Result<Vec<Resource>> {
    assert_project_permission(project_cache.clone(), &cmd.credential, &cmd.project_id).await?;

    resource_cache
        .find(&cmd.project_id, &cmd.page, &cmd.page_size)
        .await
}

pub async fn fetch_by_id(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    cmd: FetchByIdCmd,
) -> Result<Resource> {
    assert_project_permission(project_cache.clone(), &cmd.credential, &cmd.project_id).await?;

    let Some(project) = project_cache.find_by_id(&cmd.project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    let Some(resource) = resource_cache.find_by_id(&cmd.resource_id).await? else {
        return Err(Error::CommandMalformed("invalid resource id".into()));
    };

    assert_project_resource(&project, &resource)?;

    Ok(resource)
}

pub async fn create(
    project_cache: Arc<dyn ProjectDrivenCache>,
    metadata: Arc<dyn MetadataDriven>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: CreateCmd,
) -> Result<()> {
    assert_project_permission(project_cache.clone(), &cmd.credential, &cmd.project_id).await?;

    let Some(crd) = metadata.find_by_kind(&cmd.kind).await? else {
        return Err(Error::CommandMalformed("kind not supported".into()));
    };

    let Some(project) = project_cache.find_by_id(&cmd.project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    let mut spec = cmd.spec.clone();
    if let Some(status_schema) = get_schema_from_crd(&crd, "status") {
        for (key, _) in status_schema {
            if let Ok(status_field) = key.parse::<KnownField>() {
                let value = match status_field {
                    KnownField::AuthToken => {
                        let key = build_key(&project.id, &cmd.id)?;
                        encode_key(key, &cmd.kind)?
                    }
                    KnownField::Username => {
                        let user_key = build_key(&project.id, &cmd.id)?;
                        encode_key(user_key, &cmd.kind)?
                    }
                    KnownField::Password => {
                        let password_key = build_key(&project.id, &cmd.id)?;
                        BASE64_STANDARD_NO_PAD.encode(password_key)
                    }
                };
                spec.insert(key, serde_json::Value::String(value));
            }
        }
    };

    // TODO: add data from crd to build api resource
    let evt = ResourceCreated {
        id: cmd.id,
        project_id: project.id,
        project_namespace: project.namespace,
        kind: cmd.kind.clone(),
        spec: serde_json::to_string(&spec)?,
        status: ResourceStatus::Active.to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(resource = cmd.kind, "new resource created");

    Ok(())
}

pub async fn update(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: UpdateCmd,
) -> Result<Resource> {
    let Some(resource) = resource_cache.find_by_id(&cmd.id).await? else {
        return Err(Error::CommandMalformed("invalid resource id".into()));
    };

    assert_project_permission(project_cache.clone(), &cmd.credential, &resource.project_id).await?;
    let Some(project) = project_cache.find_by_id(&resource.project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    let evt = ResourceUpdated {
        id: cmd.id.clone(),
        project_id: project.id,
        project_namespace: project.namespace,
        kind: resource.kind,
        spec_patch: serde_json::to_string(&cmd.spec)?,
        updated_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(resource = cmd.id, "resource updated");

    let Some(resource) = resource_cache.find_by_id(&cmd.id).await? else {
        return Err(Error::CommandMalformed("Missing resource".into()));
    };

    Ok(resource)
}

pub async fn delete(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: DeleteCmd,
) -> Result<()> {
    assert_project_permission(project_cache.clone(), &cmd.credential, &cmd.project_id).await?;

    let Some(project) = project_cache.find_by_id(&cmd.project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    let Some(resource) = resource_cache.find_by_id(&cmd.resource_id).await? else {
        return Err(Error::CommandMalformed("invalid resource id".into()));
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

fn assert_project_resource(project: &Project, resource: &Resource) -> Result<()> {
    if project.id != resource.project_id {
        return Err(Error::CommandMalformed("invalid resource id".into()));
    }
    Ok(())
}

pub fn build_key(project_id: &str, resource_id: &str) -> Result<Vec<u8>> {
    let argon2 = Argon2::default();
    let key = format!("{project_id}{resource_id}").as_bytes().to_vec();

    let salt = SaltString::generate(&mut OsRng);

    let mut output = vec![0; 8];
    argon2.hash_password_into(&key, salt.as_str().as_bytes(), &mut output)?;

    Ok(output)
}

pub fn encode_key(key: Vec<u8>, prefix: &str) -> Result<String> {
    let prefix = format!("dmtr_{}", prefix.to_lowercase().replace("port", ""));
    let hrp = Hrp::parse(&prefix)?;
    let bech = bech32::encode::<Bech32m>(hrp, &key)?;

    Ok(bech)
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

        if page_size >= PAGE_SIZE_MAX {
            return Err(Error::CommandMalformed(format!(
                "page_size exceeded the limit of {PAGE_SIZE_MAX}"
            )));
        }

        Ok(Self {
            credential,
            project_id,
            page,
            page_size,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FetchByIdCmd {
    pub credential: Credential,
    pub project_id: String,
    pub resource_id: String,
}

pub type Spec = serde_json::value::Map<String, serde_json::Value>;
#[derive(Debug, Clone)]
pub struct CreateCmd {
    pub credential: Credential,
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub spec: Spec,
}
impl CreateCmd {
    pub fn new(credential: Credential, project_id: String, kind: String, spec: Spec) -> Self {
        let id = Uuid::new_v4().to_string();

        Self {
            credential,
            id,
            project_id,
            kind,
            spec,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateCmd {
    pub credential: Credential,
    pub id: String,
    pub spec: Spec,
}
impl UpdateCmd {
    pub fn new(credential: Credential, id: String, spec: Spec) -> Self {
        Self {
            credential,
            id,
            spec,
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
    use chrono::DateTime;
    use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
    use mockall::mock;
    use uuid::Uuid;

    use crate::domain::event::Event;
    use crate::domain::metadata::tests::mock_crd;
    use crate::domain::project::{Project, ProjectSecret, ProjectUpdate, ProjectUser};
    use crate::domain::resource::ResourceUpdate;

    use super::*;

    mock! {
        pub FakeProjectDrivenCache { }

        #[async_trait::async_trait]
        impl ProjectDrivenCache for FakeProjectDrivenCache {
            async fn find(&self, user_id: &str, page: &u32, page_size: &u32) -> Result<Vec<Project>>;
            async fn find_by_namespace(&self, namespace: &str) -> Result<Option<Project>>;
            async fn find_by_id(&self, id: &str) -> Result<Option<Project>>;
            async fn create(&self, project: &Project) -> Result<()>;
            async fn update(&self, project: &ProjectUpdate) -> Result<()>;
            async fn delete(&self, id: &str, deleted_at: &DateTime<Utc>) -> Result<()>;
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
            async fn update(&self, resource: &ResourceUpdate) -> Result<()>;
            async fn delete(&self, id: &str, deleted_at: &DateTime<Utc>) -> Result<()>;
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
        pub FakeMetadataDrivenCrds { }

        #[async_trait::async_trait]
        impl MetadataDriven for FakeMetadataDrivenCrds {
            async fn find(&self) -> Result<Vec<CustomResourceDefinition>>;
            async fn find_by_kind(&self, kind: &str) -> Result<Option<CustomResourceDefinition>>;
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
    impl Default for FetchByIdCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                project_id: Uuid::new_v4().to_string(),
                resource_id: Uuid::new_v4().to_string(),
            }
        }
    }
    impl Default for CreateCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                kind: "CardanoNodePort".into(),
                spec: serde_json::Map::default(),
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
    async fn it_should_fetch_project_resources_by_id() {
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

        let cmd = FetchByIdCmd::default();

        let result = fetch_by_id(Arc::new(project_cache), Arc::new(resource_cache), cmd).await;

        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_fetch_project_resources_by_id_when_resource_is_from_other_project() {
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

        let cmd = FetchByIdCmd::default();

        let result = fetch_by_id(Arc::new(project_cache), Arc::new(resource_cache), cmd).await;

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

        let mut metadata = MockFakeMetadataDrivenCrds::new();
        metadata
            .expect_find_by_kind()
            .return_once(|_| Ok(Some(mock_crd())));

        let mut event = MockFakeEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = CreateCmd::default();

        let result = create(
            Arc::new(project_cache),
            Arc::new(metadata),
            Arc::new(event),
            cmd,
        )
        .await;

        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_create_resource_when_crd_doesnt_exist() {
        let mut project_cache = MockFakeProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut metadata = MockFakeMetadataDrivenCrds::new();
        metadata.expect_find_by_kind().return_once(|_| Ok(None));

        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateCmd::default();

        let result = create(
            Arc::new(project_cache),
            Arc::new(metadata),
            Arc::new(event),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_resource_when_project_doesnt_exist() {
        let mut project_cache = MockFakeProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        project_cache.expect_find_by_id().return_once(|_| Ok(None));

        let mut metadata = MockFakeMetadataDrivenCrds::new();
        metadata
            .expect_find_by_kind()
            .return_once(|_| Ok(Some(mock_crd())));

        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateCmd::default();

        let result = create(
            Arc::new(project_cache),
            Arc::new(metadata),
            Arc::new(event),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_resource_when_user_doesnt_have_permission() {
        let mut project_cache = MockFakeProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let metadata = MockFakeMetadataDrivenCrds::new();
        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateCmd::default();

        let result = create(
            Arc::new(project_cache),
            Arc::new(metadata),
            Arc::new(event),
            cmd,
        )
        .await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_resource_when_secret_doesnt_have_permission() {
        let project_cache = MockFakeProjectDrivenCache::new();
        let metadata = MockFakeMetadataDrivenCrds::new();
        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = create(
            Arc::new(project_cache),
            Arc::new(metadata),
            Arc::new(event),
            cmd,
        )
        .await;

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
