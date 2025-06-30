use std::sync::Arc;

use crate::domain::{
    auth::{assert_permission, Credential},
    error::Error,
    project::cache::ProjectDrivenCache,
    resource::cache::ResourceDrivenCache,
    Result,
};

use super::{Signer, WorkerSignerDrivenStorage};

pub async fn list(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    signer_storage: Arc<dyn WorkerSignerDrivenStorage>,
    cmd: ListCmd,
) -> Result<Vec<Signer>> {
    let Some(resource) = resource_cache.find_by_id(&cmd.worker_id).await? else {
        return Err(Error::CommandMalformed("invalid resource id".into()));
    };

    assert_permission(
        project_cache.clone(),
        &cmd.credential,
        &resource.project_id,
        None,
    )
    .await?;

    let values = signer_storage.list(&resource.name).await?;

    Ok(values)
}

pub async fn get_public_key(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    signer_storage: Arc<dyn WorkerSignerDrivenStorage>,
    cmd: GetPublicKeyCmd,
) -> Result<Option<Vec<u8>>> {
    let Some(resource) = resource_cache.find_by_id(&cmd.worker_id).await? else {
        return Err(Error::CommandMalformed("invalid resource id".into()));
    };

    assert_permission(
        project_cache.clone(),
        &cmd.credential,
        &resource.project_id,
        None,
    )
    .await?;

    signer_storage
        .get_public_key(&resource.name, cmd.key_name)
        .await
}

pub async fn sign_payload(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    signer_storage: Arc<dyn WorkerSignerDrivenStorage>,
    cmd: SignPayloadCmd,
) -> Result<Vec<u8>> {
    let Some(resource) = resource_cache.find_by_id(&cmd.worker_id).await? else {
        return Err(Error::CommandMalformed("invalid resource id".into()));
    };

    assert_permission(
        project_cache.clone(),
        &cmd.credential,
        &resource.project_id,
        None,
    )
    .await?;

    signer_storage
        .sign_payload(&resource.name, cmd.key_name, cmd.payload)
        .await
}

#[derive(Debug, Clone)]
pub struct ListCmd {
    pub credential: Credential,
    pub worker_id: String,
}
impl ListCmd {
    pub fn new(credential: Credential, worker_id: String) -> Self {
        Self {
            credential,
            worker_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GetPublicKeyCmd {
    pub credential: Credential,
    pub worker_id: String,
    pub key_name: String,
}
impl GetPublicKeyCmd {
    pub fn new(credential: Credential, worker_id: String, key_name: String) -> Self {
        Self {
            credential,
            worker_id,
            key_name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SignPayloadCmd {
    pub credential: Credential,
    pub worker_id: String,
    pub key_name: String,
    pub payload: Vec<u8>,
}
impl SignPayloadCmd {
    pub fn new(
        credential: Credential,
        worker_id: String,
        key_name: String,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            credential,
            worker_id,
            key_name,
            payload,
        }
    }
}

#[cfg(test)]
mod list_tests {
    use uuid::Uuid;

    use crate::domain::{
        project::{cache::MockProjectDrivenCache, ProjectUser},
        resource::{cache::MockResourceDrivenCache, Resource},
        worker::signer::MockWorkerSignerDrivenStorage,
    };

    use super::*;

    impl Default for ListCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                worker_id: Uuid::new_v4().to_string(),
            }
        }
    }

    #[tokio::test]
    async fn should_list_worker_signer() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut storage = MockWorkerSignerDrivenStorage::new();
        storage
            .expect_list()
            .return_once(|_| Ok(vec![Signer::default()]));

        let cmd = ListCmd::default();

        let result = list(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn should_fail_to_fetch_worker_signer_without_user_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let storage = MockWorkerSignerDrivenStorage::new();

        let cmd = ListCmd::default();

        let result = list(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn should_fail_to_fetch_worker_signer_without_secret_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let project_cache = MockProjectDrivenCache::new();
        let storage = MockWorkerSignerDrivenStorage::new();

        let cmd = ListCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = list(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod get_public_key_tests {
    use uuid::Uuid;

    use crate::domain::{
        project::{cache::MockProjectDrivenCache, ProjectUser},
        resource::{cache::MockResourceDrivenCache, Resource},
        worker::signer::MockWorkerSignerDrivenStorage,
    };

    use super::*;

    impl Default for GetPublicKeyCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                worker_id: Uuid::new_v4().to_string(),
                key_name: "key_name".to_string(),
            }
        }
    }

    #[tokio::test]
    async fn should_get_public_key_worker_signer() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut storage = MockWorkerSignerDrivenStorage::new();
        storage
            .expect_get_public_key()
            .return_once(|_, _| Ok(Some(vec![])));

        let cmd = GetPublicKeyCmd {
            ..Default::default()
        };

        let result = get_public_key(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn should_fail_to_get_public_key_worker_signer_without_user_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let storage = MockWorkerSignerDrivenStorage::new();

        let cmd = GetPublicKeyCmd {
            ..Default::default()
        };

        let result = get_public_key(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn should_fail_to_get_public_key_worker_signer_without_secret_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let project_cache = MockProjectDrivenCache::new();
        let storage = MockWorkerSignerDrivenStorage::new();

        let cmd = GetPublicKeyCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = get_public_key(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }
}

#[cfg(test)]
mod sign_payload_tests {
    use uuid::Uuid;

    use crate::domain::{
        project::{cache::MockProjectDrivenCache, ProjectUser},
        resource::{cache::MockResourceDrivenCache, Resource},
        worker::signer::MockWorkerSignerDrivenStorage,
    };

    use super::*;

    impl Default for SignPayloadCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                worker_id: Uuid::new_v4().to_string(),
                key_name: "key_name".into(),
                payload: Default::default(),
            }
        }
    }

    #[tokio::test]
    async fn should_sign_payload_worker_signer() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut storage = MockWorkerSignerDrivenStorage::new();
        storage
            .expect_sign_payload()
            .return_once(|_, _, _| Ok(vec![]));

        let cmd = SignPayloadCmd::default();

        let result = sign_payload(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn should_fail_to_sign_payload_worker_signer_without_user_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let storage = MockWorkerSignerDrivenStorage::new();

        let cmd = SignPayloadCmd::default();

        let result = sign_payload(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn should_fail_to_sign_payload_worker_signer_without_secret_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let project_cache = MockProjectDrivenCache::new();
        let storage = MockWorkerSignerDrivenStorage::new();

        let cmd = SignPayloadCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = sign_payload(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }
}
