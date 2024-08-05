use std::sync::Arc;

use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use bech32::{Bech32m, Hrp};
use chrono::Utc;
use rand::{
    distributions::{Alphanumeric, DistString},
    rngs::OsRng,
};
use tracing::{error, info};
use uuid::Uuid;

use crate::domain::{
    auth::{Credential, UserId},
    error::Error,
    event::{EventDrivenBridge, ProjectCreated, ProjectSecretCreated},
    project::ProjectStatus,
    utils, Result, MAX_SECRET, PAGE_SIZE_DEFAULT, PAGE_SIZE_MAX,
};

use super::{cache::ProjectDrivenCache, Project, ProjectSecret};

pub async fn fetch(cache: Arc<dyn ProjectDrivenCache>, cmd: FetchCmd) -> Result<Vec<Project>> {
    let user_id = assert_credential(&cmd.credential)?;

    cache.find(&user_id, &cmd.page, &cmd.page_size).await
}

pub async fn create(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: CreateCmd,
) -> Result<()> {
    let user_id = assert_credential(&cmd.credential)?;

    if cache.find_by_namespace(&cmd.namespace).await?.is_some() {
        return Err(Error::CommandMalformed("invalid project namespace".into()));
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

pub async fn create_secret(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: CreateSecretCmd,
) -> Result<String> {
    assert_credential(&cmd.credential)?;
    assert_permission(cache.clone(), &cmd.credential, &cmd.project_id).await?;

    let Some(project) = cache.find_by_id(&cmd.project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    let secrets = cache.find_secret_by_project_id(&cmd.project_id).await?;
    if secrets.len() >= MAX_SECRET {
        return Err(Error::SecretExceeded(format!(
            "secrets exceeded the limit of {MAX_SECRET}"
        )));
    }

    let key = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
    let salt_string = SaltString::generate(&mut OsRng);
    let secret = cmd.secret.into_bytes();

    let argon2 = match Argon2::new_with_secret(
        &secret,
        Default::default(),
        Default::default(),
        Default::default(),
    ) {
        Ok(argon2) => argon2.clone(),
        Err(error) => {
            error!(?error, "error to configure argon2 with secret");
            return Err(Error::Unexpected("error to create the secret".into()));
        }
    };

    let key_bytes = key.into_bytes();

    let password_hash = argon2.hash_password(&key_bytes, salt_string.as_salt())?;

    let hrp = Hrp::parse("dmtr_apikey")?;
    let key = bech32::encode::<Bech32m>(hrp, &key_bytes)?;

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

pub async fn verify_secret(
    cache: Arc<dyn ProjectDrivenCache>,
    cmd: VerifySecretCmd,
) -> Result<ProjectSecret> {
    let (hrp, key) = bech32::decode(&cmd.key).map_err(|error| {
        error!(?error, "invalid bech32");
        Error::Unauthorized("invalid bech32".into())
    })?;

    if !hrp.to_string().eq("dmtr_apikey") {
        error!(?hrp, "invalid bech32 hrp");
        return Err(Error::Unauthorized("invalid project secret".into()));
    }

    let secrets = cache.find_secret_by_project_id(&cmd.project_id).await?;

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
                project_id = cmd.project_id,
                secret_id = project_secret.id,
                "error to decode phc"
            );
            return false;
        };

        argon2.verify_password(&key, &password_hash).is_ok()
    });

    let Some(secret) = secret else {
        return Err(Error::Unauthorized("invalid project secret".into()));
    };

    Ok(secret)
}

fn assert_credential(credential: &Credential) -> Result<UserId> {
    match credential {
        Credential::Auth0(user_id) => Ok(user_id.into()),
        Credential::ApiKey(_) => Err(Error::Unauthorized(
            "project rpc doesnt support secret".into(),
        )),
    }
}
async fn assert_permission(
    cache: Arc<dyn ProjectDrivenCache>,
    credential: &Credential,
    project_id: &str,
) -> Result<()> {
    match credential {
        Credential::Auth0(user_id) => {
            let result = cache.find_user_permission(user_id, project_id).await?;
            if result.is_none() {
                return Err(Error::Unauthorized("user doesnt have permission".into()));
            }

            Ok(())
        }
        Credential::ApiKey(_) => Err(Error::Unauthorized("rpc doesnt support api-key".into())),
    }
}

#[derive(Debug, Clone)]
pub struct FetchCmd {
    pub credential: Credential,
    pub page: u32,
    pub page_size: u32,
}
impl FetchCmd {
    pub fn new(credential: Credential, page: Option<u32>, page_size: Option<u32>) -> Result<Self> {
        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(PAGE_SIZE_DEFAULT);

        if page_size >= PAGE_SIZE_MAX {
            return Err(Error::CommandMalformed(format!(
                "page_size exceeded the limit of {PAGE_SIZE_MAX}"
            )));
        }

        Ok(Self {
            credential,
            page,
            page_size,
        })
    }
}
#[derive(Debug, Clone)]
pub struct CreateCmd {
    pub credential: Credential,
    pub id: String,
    pub name: String,
    pub namespace: String,
}
impl CreateCmd {
    pub fn new(credential: Credential, name: String) -> Self {
        let id = Uuid::new_v4().to_string();
        let namespace = format!("prj-{}", utils::get_random_name());

        Self {
            credential,
            id,
            name,
            namespace,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateSecretCmd {
    pub credential: Credential,
    pub secret: String,
    pub id: String,
    pub project_id: String,
    pub name: String,
}
impl CreateSecretCmd {
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
#[derive(Debug, Clone)]
pub struct VerifySecretCmd {
    pub project_id: String,
    pub key: String,
}

#[cfg(test)]
mod tests {
    use mockall::mock;
    use uuid::Uuid;

    use super::*;
    use crate::domain::{
        event::Event,
        project::ProjectUser,
        tests::{INVALID_HRP_KEY, INVALID_KEY, KEY, SECRET},
    };

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
                name: "New Project".into(),
                namespace: "sonic-vegas".into(),
            }
        }
    }
    impl Default for CreateSecretCmd {
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
    impl Default for VerifySecretCmd {
        fn default() -> Self {
            Self {
                project_id: Default::default(),
                key: KEY.into(),
            }
        }
    }

    #[tokio::test]
    async fn it_should_fetch_user_projects() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find()
            .return_once(|_, _, _| Ok(vec![Project::default()]));

        let cmd = FetchCmd::default();

        let result = fetch(Arc::new(cache), cmd).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_create_project() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache.expect_find_by_namespace().return_once(|_| Ok(None));

        let mut event = MockFakeEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = CreateCmd::default();

        let result = create(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_when_namespace_exists() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_by_namespace()
            .return_once(|_| Ok(Some(Project::default())));

        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateCmd::default();

        let result = create(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_when_invalid_permission() {
        let cache = MockFakeProjectDrivenCache::new();
        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateCmd {
            credential: Credential::ApiKey("xxxx".into()),
            ..Default::default()
        };

        let result = create(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_create_project_secret() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));
        cache
            .expect_find_secret_by_project_id()
            .return_once(|_| Ok(Vec::new()));

        let mut event = MockFakeEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = CreateSecretCmd::default();

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_secret_when_project_doesnt_exists() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache.expect_find_by_id().return_once(|_| Ok(None));

        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateSecretCmd::default();

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_secret_when_invalid_credential() {
        let cache = MockFakeProjectDrivenCache::new();
        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateSecretCmd {
            credential: Credential::ApiKey("xxxx".into()),
            ..Default::default()
        };

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_secret_when_invalid_permission() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateSecretCmd::default();

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_secret_when_max_secret_exceeded() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));
        cache
            .expect_find_secret_by_project_id()
            .return_once(|_| Ok(vec![ProjectSecret::default(); 3]));

        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateSecretCmd::default();

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_verify_secret() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_secret_by_project_id()
            .return_once(|_| Ok(vec![ProjectSecret::default()]));

        let cmd = VerifySecretCmd::default();

        let result = verify_secret(Arc::new(cache), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_verify_secret_when_invalid_key() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_secret_by_project_id()
            .return_once(|_| Ok(vec![ProjectSecret::default()]));

        let cmd = VerifySecretCmd {
            key: INVALID_KEY.into(),
            ..Default::default()
        };

        let result = verify_secret(Arc::new(cache), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_verify_secret_when_invalid_bech32() {
        let cache = MockFakeProjectDrivenCache::new();

        let cmd = VerifySecretCmd {
            key: "invalid bech32".into(),
            ..Default::default()
        };

        let result = verify_secret(Arc::new(cache), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_verify_secret_when_invalid_bech32_hrp() {
        let cache = MockFakeProjectDrivenCache::new();

        let cmd = VerifySecretCmd {
            key: INVALID_HRP_KEY.into(),
            ..Default::default()
        };

        let result = verify_secret(Arc::new(cache), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_verify_secret_when_there_arent_secrets_storaged() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_secret_by_project_id()
            .return_once(|_| Ok(vec![]));

        let cmd = VerifySecretCmd::default();

        let result = verify_secret(Arc::new(cache), cmd).await;
        assert!(result.is_err());
    }
}
