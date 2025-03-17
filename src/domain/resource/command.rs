use std::sync::Arc;

use argon2::{password_hash::SaltString, Argon2};
use base64::{prelude::BASE64_STANDARD_NO_PAD, Engine};
use bech32::{Bech32m, Hrp};
use chrono::Utc;
use rand::rngs::OsRng;
use tracing::{error, info};
use uuid::Uuid;

use crate::domain::{
    auth::{assert_permission, Credential},
    error::Error,
    event::{EventDrivenBridge, ResourceCreated, ResourceDeleted},
    metadata::{KnownField, MetadataDriven},
    project::cache::ProjectDrivenCache,
    resource::{ResourceStatus, ResourceUpdated},
    utils::{self, get_schema_from_crd},
    Result, PAGE_SIZE_DEFAULT, PAGE_SIZE_MAX,
};

use super::{cache::ResourceDrivenCache, Resource};

pub async fn fetch(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    metadata: Arc<dyn MetadataDriven>,
    cmd: FetchCmd,
) -> Result<Vec<Resource>> {
    assert_permission(
        project_cache.clone(),
        &cmd.credential,
        &cmd.project_id,
        None,
    )
    .await?;

    let resources = resource_cache
        .find(&cmd.project_id, &cmd.page, &cmd.page_size)
        .await?
        .into_iter()
        .map(|mut resource| {
            match metadata.render_hbs(&resource.kind, &resource.spec) {
                Ok(annotations) => resource.annotations = Some(annotations),
                Err(error) => error!(?error),
            };

            resource
        })
        .collect();

    Ok(resources)
}

pub async fn fetch_by_id(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    metadata: Arc<dyn MetadataDriven>,
    cmd: FetchByIdCmd,
) -> Result<Resource> {
    let Some(mut resource) = resource_cache.find_by_id(&cmd.id).await? else {
        return Err(Error::CommandMalformed("invalid resource id".into()));
    };

    assert_permission(
        project_cache.clone(),
        &cmd.credential,
        &resource.project_id,
        None,
    )
    .await?;

    match metadata.render_hbs(&resource.kind, &resource.spec) {
        Ok(annotations) => resource.annotations = Some(annotations),
        Err(error) => error!(?error),
    };

    Ok(resource)
}

pub async fn create(
    resource_cache: Arc<dyn ResourceDrivenCache>,
    project_cache: Arc<dyn ProjectDrivenCache>,
    metadata: Arc<dyn MetadataDriven>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: CreateCmd,
) -> Result<()> {
    assert_permission(
        project_cache.clone(),
        &cmd.credential,
        &cmd.project_id,
        None,
    )
    .await?;

    if resource_cache
        .find_by_name(&cmd.project_id, &cmd.name)
        .await?
        .is_some()
    {
        return Err(Error::Unexpected("invalid random name, try again".into()));
    }

    let Some(metadata) = metadata.find_by_kind(&cmd.kind)? else {
        return Err(Error::CommandMalformed("kind not supported".into()));
    };

    let Some(project) = project_cache.find_by_id(&cmd.project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    let mut spec = cmd.spec.clone();
    if let Some(status_schema) = get_schema_from_crd(&metadata.crd, "status") {
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
        name: cmd.name,
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

    assert_permission(
        project_cache.clone(),
        &cmd.credential,
        &resource.project_id,
        None,
    )
    .await?;

    let Some(project) = project_cache.find_by_id(&resource.project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    let evt = ResourceUpdated {
        id: cmd.id.clone(),
        project_id: project.id,
        project_namespace: project.namespace,
        name: resource.name,
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
    let Some(resource) = resource_cache.find_by_id(&cmd.id).await? else {
        return Err(Error::CommandMalformed("invalid resource id".into()));
    };

    assert_permission(
        project_cache.clone(),
        &cmd.credential,
        &resource.project_id,
        None,
    )
    .await?;

    let Some(project) = project_cache.find_by_id(&resource.project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    let evt = ResourceDeleted {
        id: cmd.id,
        project_id: project.id,
        project_namespace: project.namespace,
        name: resource.name,
        kind: resource.kind.clone(),
        status: ResourceStatus::Deleted.to_string(),
        deleted_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(resource = resource.kind, "resource deleted");

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
    let hrp = Hrp::parse(&prefix.to_lowercase().replace("port", ""))?;
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
    pub id: String,
}

pub type Spec = serde_json::value::Map<String, serde_json::Value>;
#[derive(Debug, Clone)]
pub struct CreateCmd {
    pub credential: Credential,
    pub id: String,
    pub name: String,
    pub project_id: String,
    pub kind: String,
    pub spec: Spec,
}
impl CreateCmd {
    pub fn new(
        credential: Credential,
        project_id: String,
        kind: String,
        spec: String,
    ) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let name = format!(
            "{}-{}",
            kind.to_lowercase().replace("port", ""),
            utils::get_random_salt()
        );

        let value = serde_json::from_str(&spec)
            .map_err(|_| Error::CommandMalformed("spec must be a json".into()))?;
        let spec = match value {
            serde_json::Value::Object(v) => Ok(v),
            _ => Err(Error::CommandMalformed("invalid spec json".into())),
        }?;

        Ok(Self {
            credential,
            id,
            name,
            project_id,
            kind,
            spec,
        })
    }
}

#[derive(Debug, Clone)]
pub struct UpdateCmd {
    pub credential: Credential,
    pub id: String,
    pub spec: Spec,
}
impl UpdateCmd {
    pub fn new(credential: Credential, id: String, spec: String) -> Result<Self> {
        let value = serde_json::from_str(&spec)
            .map_err(|_| Error::CommandMalformed("spec must be a json".into()))?;
        let spec = match value {
            serde_json::Value::Object(v) => Ok(v),
            _ => Err(Error::CommandMalformed("invalid spec json".into())),
        }?;

        Ok(Self {
            credential,
            id,
            spec,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DeleteCmd {
    pub credential: Credential,
    pub id: String,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::domain::event::MockEventDrivenBridge;
    use crate::domain::metadata::{MockMetadataDriven, ResourceMetadata};
    use crate::domain::project::cache::MockProjectDrivenCache;
    use crate::domain::project::{Project, ProjectUser};
    use crate::domain::resource::cache::MockResourceDrivenCache;

    use super::*;

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
                id: Uuid::new_v4().to_string(),
            }
        }
    }
    impl Default for CreateCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: Uuid::new_v4().to_string(),
                name: format!("cardanonode-{}", utils::get_random_salt()),
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
                id: Uuid::new_v4().to_string(),
            }
        }
    }

    #[tokio::test]
    async fn it_should_fetch_project_resources() {
        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find()
            .return_once(|_, _, _| Ok(vec![Resource::default()]));

        let mut metadata = MockMetadataDriven::new();
        metadata
            .expect_render_hbs()
            .return_once(|_, _| Ok("[{}]".into()));

        let cmd = FetchCmd::default();

        let result = fetch(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(metadata),
            cmd,
        )
        .await;

        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_fetch_project_resources_when_user_doesnt_have_permission() {
        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let resource_cache = MockResourceDrivenCache::new();

        let metadata = MockMetadataDriven::new();

        let cmd = FetchCmd::default();

        let result = fetch(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(metadata),
            cmd,
        )
        .await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_fetch_project_resources_when_secret_doesnt_have_permission() {
        let project_cache = MockProjectDrivenCache::new();
        let resource_cache = MockResourceDrivenCache::new();
        let metadata = MockMetadataDriven::new();

        let cmd = FetchCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = fetch(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(metadata),
            cmd,
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_fetch_project_resources_by_id() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut metadata = MockMetadataDriven::new();
        metadata
            .expect_render_hbs()
            .return_once(|_, _| Ok("[{}]".into()));

        let cmd = FetchByIdCmd::default();

        let result = fetch_by_id(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(metadata),
            cmd,
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_create_resource() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_name()
            .return_once(|_, _| Ok(None));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        project_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let mut metadata = MockMetadataDriven::new();
        metadata
            .expect_find_by_kind()
            .return_once(|_| Ok(Some(ResourceMetadata::default())));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = CreateCmd::default();

        let result = create(
            Arc::new(resource_cache),
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
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_name()
            .return_once(|_, _| Ok(None));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut metadata = MockMetadataDriven::new();
        metadata.expect_find_by_kind().return_once(|_| Ok(None));

        let event = MockEventDrivenBridge::new();

        let cmd = CreateCmd::default();

        let result = create(
            Arc::new(resource_cache),
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
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_name()
            .return_once(|_, _| Ok(None));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        project_cache.expect_find_by_id().return_once(|_| Ok(None));

        let mut metadata = MockMetadataDriven::new();
        metadata
            .expect_find_by_kind()
            .return_once(|_| Ok(Some(ResourceMetadata::default())));

        let event = MockEventDrivenBridge::new();

        let cmd = CreateCmd::default();

        let result = create(
            Arc::new(resource_cache),
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
        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let resource_cache = MockResourceDrivenCache::new();
        let metadata = MockMetadataDriven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = CreateCmd::default();

        let result = create(
            Arc::new(resource_cache),
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
        let resource_cache = MockResourceDrivenCache::new();
        let project_cache = MockProjectDrivenCache::new();
        let metadata = MockMetadataDriven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = CreateCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = create(
            Arc::new(resource_cache),
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
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        project_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let mut event = MockEventDrivenBridge::new();
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
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let event = MockEventDrivenBridge::new();

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
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let project_cache = MockProjectDrivenCache::new();
        let event = MockEventDrivenBridge::new();

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
}
