use anyhow::{bail, Error, Result};
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use bech32::{Bech32m, Hrp};
use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::Namespace;
use kube::{api::ObjectMeta, ResourceExt};
use rand::{
    distributions::{Alphanumeric, DistString},
    rngs::OsRng,
    Rng,
};
use rdkafka::message::ToBytes;
use std::{fmt::Display, str::FromStr, sync::Arc};
use tracing::{error, info};
use uuid::Uuid;

use super::{
    auth::{Credential, UserId},
    event::{EventDrivenBridge, ProjectCreated, ProjectSecretCreated},
};

pub async fn create(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: CreateProjectCmd,
) -> Result<()> {
    let user_id = assert_permission(cmd.credential)?;

    if cache.find_by_namespace(&cmd.namespace).await?.is_some() {
        bail!("invalid project namespace")
    }

    let evt = ProjectCreated {
        id: cmd.id,
        namespace: cmd.namespace.clone(),
        name: cmd.name,
        owner: user_id,
        status: ProjectStatus::Active.to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(project = cmd.namespace, "new project created");

    Ok(())
}

pub async fn create_cache(cache: Arc<dyn ProjectDrivenCache>, evt: ProjectCreated) -> Result<()> {
    cache.create(&evt.try_into()?).await?;

    Ok(())
}

pub async fn apply_manifest(
    cluster: Arc<dyn ProjectDrivenCluster>,
    evt: ProjectCreated,
) -> Result<()> {
    if cluster.find_by_name(&evt.namespace).await?.is_some() {
        bail!("namespace alread exist")
    }

    let namespace = Namespace {
        metadata: ObjectMeta {
            name: Some(evt.namespace),
            ..Default::default()
        },
        ..Default::default()
    };

    cluster.create(&namespace).await?;

    //TODO: create event to update cache
    info!(namespace = namespace.name_any(), "new namespace created");

    Ok(())
}

pub async fn create_secret(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: CreateProjectSecretCmd,
) -> Result<String> {
    assert_permission(cmd.credential)?;

    let Some(project) = cache.find_by_id(&cmd.project_id).await? else {
        bail!("project doesnt exist")
    };

    let key = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
    let salt_string = SaltString::generate(&mut OsRng);
    let secret = cmd.secret.to_bytes();

    let argon2 = match Argon2::new_with_secret(
        secret,
        Default::default(),
        Default::default(),
        Default::default(),
    ) {
        Ok(argon2) => argon2.clone(),
        Err(error) => {
            error!(?error, "error to configure argon2 with secret");
            bail!("internal error")
        }
    };

    let password_hash = argon2
        .hash_password(key.to_bytes(), salt_string.as_salt())
        .map_err(|err| Error::msg(err.to_string()))?;

    let hrp = Hrp::parse("dmtr_apikey")?;
    let key = bech32::encode::<Bech32m>(hrp, key.to_bytes())?;

    let evt = ProjectSecretCreated {
        id: cmd.id,
        project_id: project.id,
        name: cmd.name,
        phc: password_hash.to_string(),
        secret: secret.to_vec(),
        created_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!("new project secret created");

    Ok(key)
}
pub async fn create_secret_cache(
    cache: Arc<dyn ProjectDrivenCache>,
    evt: ProjectSecretCreated,
) -> Result<()> {
    cache.create_secret(&evt.into()).await?;

    Ok(())
}
pub async fn verify_secret(
    cache: Arc<dyn ProjectDrivenCache>,
    project_id: &str,
    key: &str,
) -> Result<ProjectSecretCache> {
    let (hrp, key) = bech32::decode(key).map_err(|error| {
        error!(?error, "invalid bech32");
        Error::msg("invalid bech32")
    })?;

    if !hrp.to_string().eq("dmtr_apikey") {
        error!(?hrp, "invalid bech32 hrp");
        bail!("invalid project secret")
    }

    let secrets = cache.find_secret_by_project_id(project_id).await?;

    let secret = secrets.into_iter().find(|project_secret| {
        let argon2 = Argon2::new_with_secret(
            &project_secret.secret,
            Default::default(),
            Default::default(),
            Default::default(),
        )
        .unwrap();

        let Ok(password_hash) = PasswordHash::new(&project_secret.phc) else {
            error!(
                project_id,
                secret_id = project_secret.id,
                "error to decode phc"
            );
            return false;
        };

        argon2.verify_password(&key, &password_hash).is_ok()
    });

    let Some(secret) = secret else {
        bail!("invalid project secret");
    };

    Ok(secret)
}

fn assert_permission(credential: Credential) -> Result<UserId> {
    match credential {
        Credential::Auth0(user_id) => Ok(user_id),
        Credential::ApiKey(_) => bail!("rpc doesnt support api-key"),
    }
}

#[derive(Debug, Clone)]
pub struct CreateProjectCmd {
    pub credential: Credential,
    pub id: String,
    pub name: String,
    pub namespace: String,
}
impl CreateProjectCmd {
    pub fn new(credential: Credential, name: String) -> Self {
        let id = Uuid::new_v4().to_string();
        let namespace: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();
        let namespace = format!("prj-{}", namespace.to_lowercase());

        Self {
            credential,
            id,
            name,
            namespace,
        }
    }
}

#[derive(Debug)]
pub enum ProjectStatus {
    Active,
    Deleted,
}
impl FromStr for ProjectStatus {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "active" => Ok(ProjectStatus::Active),
            "deleted" => Ok(ProjectStatus::Deleted),
            _ => bail!("project status not supported"),
        }
    }
}
impl Display for ProjectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectStatus::Active => write!(f, "active"),
            ProjectStatus::Deleted => write!(f, "deleted"),
        }
    }
}

#[derive(Debug)]
pub struct ProjectCache {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub owner: String,
    pub status: ProjectStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl TryFrom<ProjectCreated> for ProjectCache {
    type Error = Error;

    fn try_from(value: ProjectCreated) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            namespace: value.namespace,
            name: value.name,
            owner: value.owner,
            status: value.status.parse()?,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CreateProjectSecretCmd {
    pub credential: Credential,
    pub secret: String,
    pub id: String,
    pub project_id: String,
    pub name: String,
}
impl CreateProjectSecretCmd {
    pub fn new(credential: Credential, secret: String, project_id: String, name: String) -> Self {
        let id = Uuid::new_v4().to_string();

        Self {
            credential,
            secret,
            id,
            project_id,
            name,
        }
    }
}

#[derive(Debug)]
pub struct ProjectSecretCache {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub phc: String,
    pub secret: Vec<u8>,
    pub created_at: DateTime<Utc>,
}
impl From<ProjectSecretCreated> for ProjectSecretCache {
    fn from(value: ProjectSecretCreated) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            name: value.name,
            phc: value.phc,
            secret: value.secret,
            created_at: value.created_at,
        }
    }
}

#[allow(dead_code)]
pub struct ProjectUserCache {
    pub user_id: String,
    pub project_id: String,
    pub created_at: DateTime<Utc>,
}

#[async_trait::async_trait]
pub trait ProjectDrivenCache: Send + Sync {
    async fn find(&self, user_id: &str) -> Result<Vec<ProjectCache>>;
    async fn find_by_namespace(&self, namespace: &str) -> Result<Option<ProjectCache>>;
    async fn find_by_id(&self, id: &str) -> Result<Option<ProjectCache>>;
    async fn create(&self, project: &ProjectCache) -> Result<()>;
    async fn create_secret(&self, secret: &ProjectSecretCache) -> Result<()>;
    async fn find_secret_by_project_id(&self, project_id: &str) -> Result<Vec<ProjectSecretCache>>;
    async fn find_user_permission(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Option<ProjectUserCache>>;
}

#[async_trait::async_trait]
pub trait ProjectDrivenCluster: Send + Sync {
    async fn create(&self, namespace: &Namespace) -> Result<()>;
    async fn find_by_name(&self, name: &str) -> Result<Option<Namespace>>;
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::Namespace;
    use mockall::mock;
    use uuid::Uuid;

    use super::*;
    use crate::domain::event::Event;

    mock! {
        pub FakeProjectDrivenCache { }

        #[async_trait::async_trait]
        impl ProjectDrivenCache for FakeProjectDrivenCache {
            async fn find(&self, user_id: &str) -> Result<Vec<ProjectCache>>;
            async fn find_by_namespace(&self, namespace: &str) -> Result<Option<ProjectCache>>;
            async fn find_by_id(&self, id: &str) -> Result<Option<ProjectCache>>;
            async fn create(&self, project: &ProjectCache) -> Result<()>;
            async fn create_secret(&self, secret: &ProjectSecretCache) -> Result<()>;
            async fn find_secret_by_project_id(&self, project_id: &str) -> Result<Vec<ProjectSecretCache>>;
            async fn find_user_permission(&self,user_id: &str, project_id: &str) -> Result<Option<ProjectUserCache>>;
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
        pub FakeProjectDrivenCluster { }

        #[async_trait::async_trait]
        impl ProjectDrivenCluster for FakeProjectDrivenCluster {
            async fn create(&self, namespace: &Namespace) -> Result<()>;
            async fn find_by_name(&self, name: &str) -> Result<Option<Namespace>>;
        }
    }

    impl Default for CreateProjectCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: Uuid::new_v4().to_string(),
                name: "New Project".into(),
                namespace: "sonic-vegas".into(),
            }
        }
    }
    impl Default for ProjectCache {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                name: "New Project".into(),
                namespace: "sonic-vegas".into(),
                owner: "user id".into(),
                status: ProjectStatus::Active,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        }
    }
    impl Default for ProjectCreated {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                name: "New Project".into(),
                namespace: "sonic-vegas".into(),
                owner: "user id".into(),
                status: ProjectStatus::Active.to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        }
    }

    const KEY: &str = "dmtr_apikey1g9gyswtcf3zxwd26v4x5jj3jw5wx3sn2";
    const PHC: &str = "$argon2id$v=19$m=19456,t=2,p=1$xVIt6Wr/bm1FewVhTr6zgA$nTO6EgGeOYZe7thACrHmFUWND40U4GEQCXKyvqzvRvs";
    const SECRET: &str = "fabric@txpipe";
    const INVALID_KEY: &str = "dmtr_apikey1xe6xzcjxv9nhycnz2ffnq6m02y7nat9e";
    const INVALID_HRP_KEY: &str = "dmtr_test18pp5vkjzfuuyzwpeg9gk2a2zvsylc5wg";

    impl Default for CreateProjectSecretCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                name: "Key 1".into(),
                secret: SECRET.into(),
            }
        }
    }
    impl Default for ProjectSecretCache {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                name: "Key 1".into(),
                phc: PHC.into(),
                secret: SECRET.to_bytes().to_vec(),
                created_at: Utc::now(),
            }
        }
    }
    impl Default for ProjectSecretCreated {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                name: "Key 1".into(),
                phc: PHC.into(),
                secret: SECRET.to_bytes().to_vec(),
                created_at: Utc::now(),
            }
        }
    }

    impl Default for ProjectUserCache {
        fn default() -> Self {
            Self {
                user_id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                created_at: Utc::now(),
            }
        }
    }

    #[tokio::test]
    async fn it_should_create_project() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache.expect_find_by_namespace().return_once(|_| Ok(None));

        let mut event = MockFakeEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = CreateProjectCmd::default();

        let result = create(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_when_namespace_exists() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_by_namespace()
            .return_once(|_| Ok(Some(ProjectCache::default())));

        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateProjectCmd::default();

        let result = create(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_when_invalid_permission() {
        let cache = MockFakeProjectDrivenCache::new();
        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateProjectCmd {
            credential: Credential::ApiKey("xxxx".into()),
            ..Default::default()
        };

        let result = create(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_create_project_cache() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache.expect_create().return_once(|_| Ok(()));

        let evt = ProjectCreated::default();

        let result = create_cache(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_create_project_secret() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(ProjectCache::default())));

        let mut event = MockFakeEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = CreateProjectSecretCmd::default();

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_secret_when_project_doesnt_exists() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache.expect_find_by_id().return_once(|_| Ok(None));

        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateProjectSecretCmd::default();

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_secret_when_invalid_permission() {
        let cache = MockFakeProjectDrivenCache::new();
        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateProjectSecretCmd {
            credential: Credential::ApiKey("xxxx".into()),
            ..Default::default()
        };

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_create_project_secret_cache() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache.expect_create_secret().return_once(|_| Ok(()));

        let evt = ProjectSecretCreated::default();

        let result = create_secret_cache(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_verify_secret() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_secret_by_project_id()
            .return_once(|_| Ok(vec![ProjectSecretCache::default()]));

        let result = verify_secret(Arc::new(cache), Default::default(), KEY).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_verify_secret_when_invalid_key() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_secret_by_project_id()
            .return_once(|_| Ok(vec![ProjectSecretCache::default()]));

        let result = verify_secret(Arc::new(cache), Default::default(), INVALID_KEY).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_verify_secret_when_invalid_bech32() {
        let cache = MockFakeProjectDrivenCache::new();

        let result = verify_secret(Arc::new(cache), Default::default(), "invalid bech32").await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_verify_secret_when_invalid_bech32_hrp() {
        let cache = MockFakeProjectDrivenCache::new();

        let result = verify_secret(Arc::new(cache), Default::default(), INVALID_HRP_KEY).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_verify_secret_when_there_arent_secrets_storaged() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_secret_by_project_id()
            .return_once(|_| Ok(vec![]));

        let result = verify_secret(Arc::new(cache), Default::default(), KEY).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_apply_manifest() {
        let mut cluster = MockFakeProjectDrivenCluster::new();
        cluster.expect_create().return_once(|_| Ok(()));
        cluster.expect_find_by_name().return_once(|_| Ok(None));

        let project = ProjectCreated::default();

        let result = apply_manifest(Arc::new(cluster), project).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_apply_manifest_when_resource_exists() {
        let mut cluster = MockFakeProjectDrivenCluster::new();
        cluster.expect_create().return_once(|_| Ok(()));
        cluster
            .expect_find_by_name()
            .return_once(|_| Ok(Some(Namespace::default())));

        let project = ProjectCreated::default();

        let result = apply_manifest(Arc::new(cluster), project).await;
        assert!(result.is_err());
    }
}
