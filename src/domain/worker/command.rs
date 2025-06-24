use std::sync::Arc;

use crate::domain::{
    auth::{assert_permission, Credential},
    error::Error,
    project::cache::ProjectDrivenCache,
    resource::cache::ResourceDrivenCache,
    Result, PAGE_SIZE_DEFAULT, PAGE_SIZE_MAX,
};

use super::{KeyValue, WorkerKeyValueDrivenStorage};

pub async fn fetch(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    key_value_storage: Arc<dyn WorkerKeyValueDrivenStorage>,
    cmd: FetchCmd,
) -> Result<Vec<KeyValue>> {
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

    let values = key_value_storage
        .find(&cmd.worker_id, &cmd.page, &cmd.page_size)
        .await?;

    Ok(values)
}

pub async fn update(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    key_value_storage: Arc<dyn WorkerKeyValueDrivenStorage>,
    cmd: UpdateCmd,
) -> Result<()> {
    let Some(resource) = resource_cache.find_by_id(&cmd.key_value.worker_id).await? else {
        return Err(Error::CommandMalformed("invalid resource id".into()));
    };

    assert_permission(
        project_cache.clone(),
        &cmd.credential,
        &resource.project_id,
        None,
    )
    .await?;

    key_value_storage.update(&cmd.key_value).await?;

    Ok(())
}

pub async fn delete(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    key_value_storage: Arc<dyn WorkerKeyValueDrivenStorage>,
    cmd: DeleteCmd,
) -> Result<()> {
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

    key_value_storage.delete(&cmd.worker_id, &cmd.key).await?;

    Ok(())
}

#[derive(Debug, Clone)]
pub struct FetchCmd {
    pub credential: Credential,
    pub worker_id: String,
    pub page: u32,
    pub page_size: u32,
}
impl FetchCmd {
    pub fn new(
        credential: Credential,
        worker_id: String,
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
            worker_id,
            page,
            page_size,
        })
    }
}

#[derive(Debug, Clone)]
pub struct UpdateCmd {
    pub credential: Credential,
    pub key_value: KeyValue,
}
impl UpdateCmd {
    pub fn new(credential: Credential, key_value: KeyValue) -> Self {
        Self {
            credential,
            key_value,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeleteCmd {
    pub credential: Credential,
    pub worker_id: String,
    pub key: String,
}
impl DeleteCmd {
    pub fn new(credential: Credential, worker_id: String, key: String) -> Self {
        Self {
            credential,
            worker_id,
            key,
        }
    }
}

#[cfg(test)]
mod fetch_tests {
    use uuid::Uuid;

    use crate::domain::{
        project::{cache::MockProjectDrivenCache, ProjectUser},
        resource::{cache::MockResourceDrivenCache, Resource},
        worker::MockWorkerKeyValueDrivenStorage,
    };

    use super::*;

    impl Default for FetchCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                worker_id: Uuid::new_v4().to_string(),
                page: 1,
                page_size: 12,
            }
        }
    }

    #[tokio::test]
    async fn should_fetch_worker_key_value() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut storage = MockWorkerKeyValueDrivenStorage::new();
        storage
            .expect_find()
            .return_once(|_, _, _| Ok(vec![KeyValue::default()]));

        let cmd = FetchCmd::default();

        let result = fetch(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn should_fail_to_fetch_worker_key_value_without_user_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let storage = MockWorkerKeyValueDrivenStorage::new();

        let cmd = FetchCmd::default();

        let result = fetch(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn should_fail_to_fetch_worker_key_value_without_secret_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let project_cache = MockProjectDrivenCache::new();
        let storage = MockWorkerKeyValueDrivenStorage::new();

        let cmd = FetchCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = fetch(
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
mod update_tests {
    use uuid::Uuid;

    use crate::domain::{
        project::{cache::MockProjectDrivenCache, ProjectUser},
        resource::{cache::MockResourceDrivenCache, Resource},
        worker::MockWorkerKeyValueDrivenStorage,
    };

    use super::*;

    impl Default for UpdateCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                key_value: Default::default(),
            }
        }
    }

    #[tokio::test]
    async fn should_update_worker_key_value() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut storage = MockWorkerKeyValueDrivenStorage::new();
        storage.expect_update().return_once(|_| Ok(()));

        let cmd = UpdateCmd {
            key_value: KeyValue::default(),
            ..Default::default()
        };

        let result = update(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn should_fail_to_update_worker_key_value_without_user_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let storage = MockWorkerKeyValueDrivenStorage::new();

        let cmd = UpdateCmd {
            key_value: KeyValue::default(),
            ..Default::default()
        };

        let result = update(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn should_fail_to_update_worker_key_value_without_secret_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let project_cache = MockProjectDrivenCache::new();
        let storage = MockWorkerKeyValueDrivenStorage::new();

        let cmd = UpdateCmd {
            key_value: KeyValue::default(),
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = update(
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
mod delete_tests {
    use uuid::Uuid;

    use crate::domain::{
        project::{cache::MockProjectDrivenCache, ProjectUser},
        resource::{cache::MockResourceDrivenCache, Resource},
        worker::MockWorkerKeyValueDrivenStorage,
    };

    use super::*;

    impl Default for DeleteCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                worker_id: Uuid::new_v4().to_string(),
                key: "key".into(),
            }
        }
    }

    #[tokio::test]
    async fn should_delete_worker_key_value() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut storage = MockWorkerKeyValueDrivenStorage::new();
        storage.expect_delete().return_once(|_, _| Ok(()));

        let cmd = DeleteCmd::default();

        let result = delete(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn should_fail_to_delete_worker_key_value_without_user_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let storage = MockWorkerKeyValueDrivenStorage::new();

        let cmd = DeleteCmd::default();

        let result = delete(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn should_fail_to_delete_worker_key_value_without_secret_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let project_cache = MockProjectDrivenCache::new();
        let storage = MockWorkerKeyValueDrivenStorage::new();

        let cmd = DeleteCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = delete(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(storage),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }
}
