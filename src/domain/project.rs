use anyhow::{bail, Error, Result};
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use bech32::{Bech32m, Hrp};
use k8s_openapi::api::core::v1::Namespace;
use kube::{api::ObjectMeta, ResourceExt};
use rand::{
    distributions::{Alphanumeric, DistString},
    rngs::OsRng,
    Rng,
};
use rdkafka::message::ToBytes;
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

use super::{
    auth::Credential,
    event::{EventDrivenBridge, ProjectCreated, ProjectSecretCreated},
};

pub async fn create(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: CreateProjectCmd,
) -> Result<()> {
    if cache.find_by_namespace(&cmd.namespace).await?.is_some() {
        bail!("invalid project namespace")
    }

    let owner = String::new();

    let evt = ProjectCreated {
        id: cmd.id,
        namespace: cmd.namespace.clone(),
        name: cmd.name,
        owner,
    };

    event.dispatch(evt.into()).await?;
    info!(project = cmd.namespace, "new project created");

    Ok(())
}

pub async fn create_cache(cache: Arc<dyn ProjectDrivenCache>, evt: ProjectCreated) -> Result<()> {
    cache.create(&evt.into()).await?;

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
    // TODO validate credential

    let Some(project) = cache.find_by_id(&cmd.project_id).await? else {
        bail!("project doesnt exist")
    };

    let key = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
    let salt_string = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
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

    let argon2 = Argon2::default();
    let secret = secrets.into_iter().find(|secret| {
        let Ok(password_hash) = PasswordHash::new(&secret.phc) else {
            error!(project_id, secret_id = secret.id, "error to decode phc");
            return false;
        };

        argon2.verify_password(&key, &password_hash).is_ok()
    });

    let Some(secret) = secret else {
        bail!("invalid project secret");
    };

    Ok(secret)
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

pub struct ProjectCache {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub owner: String,
}
impl From<ProjectCreated> for ProjectCache {
    fn from(value: ProjectCreated) -> Self {
        Self {
            id: value.id,
            namespace: value.namespace,
            name: value.name,
            owner: value.owner,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateProjectSecretCmd {
    pub credential: Credential,
    pub id: String,
    pub project_id: String,
    pub name: String,
}
impl CreateProjectSecretCmd {
    pub fn new(credential: Credential, project_id: String, name: String) -> Self {
        let id = Uuid::new_v4().to_string();

        Self {
            credential,
            id,
            project_id,
            name,
        }
    }
}

pub struct ProjectSecretCache {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub phc: String,
}
impl From<ProjectSecretCreated> for ProjectSecretCache {
    fn from(value: ProjectSecretCreated) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            name: value.name,
            phc: value.phc,
        }
    }
}

#[async_trait::async_trait]
pub trait ProjectDrivenCache: Send + Sync {
    async fn find_by_namespace(&self, namespace: &str) -> Result<Option<ProjectCache>>;
    async fn find_by_id(&self, id: &str) -> Result<Option<ProjectCache>>;
    async fn create(&self, project: &ProjectCache) -> Result<()>;
    async fn create_secret(&self, secret: &ProjectSecretCache) -> Result<()>;
    async fn find_secret_by_project_id(&self, project_id: &str) -> Result<Vec<ProjectSecretCache>>;
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
            async fn find_by_namespace(&self, namespace: &str) -> Result<Option<ProjectCache>>;
            async fn find_by_id(&self, id: &str) -> Result<Option<ProjectCache>>;
            async fn create(&self, project: &ProjectCache) -> Result<()>;
            async fn create_secret(&self, secret: &ProjectSecretCache) -> Result<()>;
            async fn find_secret_by_project_id(&self, project_id: &str) -> Result<Vec<ProjectSecretCache>>;
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
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }

    #[tokio::test]
    async fn it_should_fail_when_project_namespace_exists() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache
            .expect_find_by_namespace()
            .return_once(|_| Ok(Some(ProjectCache::default())));

        let event = MockFakeEventDrivenBridge::new();

        let cmd = CreateProjectCmd::default();

        let result = create(Arc::new(cache), Arc::new(event), cmd).await;
        if result.is_ok() {
            unreachable!("Fail to validate when the namespace is duplicated")
        }
    }

    #[tokio::test]
    async fn it_should_create_project_cache() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache.expect_create().return_once(|_| Ok(()));

        let project = ProjectCreated::default();

        let result = create_cache(Arc::new(cache), project).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
    #[tokio::test]
    async fn it_should_apply_project_cluster() {
        let mut cluster = MockFakeProjectDrivenCluster::new();
        cluster.expect_create().return_once(|_| Ok(()));
        cluster.expect_find_by_name().return_once(|_| Ok(None));

        let project = ProjectCreated::default();

        let result = apply_manifest(Arc::new(cluster), project).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }

    #[tokio::test]
    async fn it_should_fail_when_project_resource_exists() {
        let mut cluster = MockFakeProjectDrivenCluster::new();
        cluster.expect_create().return_once(|_| Ok(()));
        cluster
            .expect_find_by_name()
            .return_once(|_| Ok(Some(Namespace::default())));

        let project = ProjectCreated::default();

        let result = apply_manifest(Arc::new(cluster), project).await;
        if result.is_ok() {
            unreachable!("Fail to validate when the namespace alread exists")
        }
    }
}
