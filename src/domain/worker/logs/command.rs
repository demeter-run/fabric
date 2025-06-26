use std::sync::Arc;

use crate::domain::{
    auth::{assert_permission, Credential},
    error::Error,
    project::cache::ProjectDrivenCache,
    resource::cache::ResourceDrivenCache,
    Result, PAGE_SIZE_DEFAULT, PAGE_SIZE_MAX,
};

use super::{FetchDirection, Log, WorkerLogsDrivenStorage};

pub async fn fetch(
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    logs_storage: Arc<dyn WorkerLogsDrivenStorage>,
    cmd: FetchCmd,
) -> Result<Vec<Log>> {
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

    let logs = match cmd.direction {
        FetchDirection::Prev => {
            logs_storage
                .prev(&resource.name, cmd.cursor, cmd.limit)
                .await?
        }
        FetchDirection::Next => {
            logs_storage
                .next(&resource.name, cmd.cursor, cmd.limit)
                .await?
        }
    };

    Ok(logs)
}

#[derive(Debug, Clone)]
pub struct FetchCmd {
    pub credential: Credential,
    pub worker_id: String,
    pub cursor: i64,
    pub limit: i64,
    pub direction: FetchDirection,
}
impl FetchCmd {
    pub fn new(
        credential: Credential,
        worker_id: String,
        cursor: i64,
        direction: Option<FetchDirection>,
        limit: Option<i64>,
    ) -> Result<Self> {
        let limit = limit.unwrap_or(PAGE_SIZE_DEFAULT as i64);
        let direction = direction.unwrap_or(FetchDirection::Next);

        if limit >= PAGE_SIZE_MAX as i64 {
            return Err(Error::CommandMalformed(format!(
                "limit exceeded the maximum of {PAGE_SIZE_MAX}"
            )));
        }

        Ok(Self {
            credential,
            worker_id,
            cursor,
            direction,
            limit,
        })
    }
}

#[cfg(test)]
mod fetch_tests {
    use uuid::Uuid;

    use crate::domain::{
        project::{cache::MockProjectDrivenCache, ProjectUser},
        resource::{cache::MockResourceDrivenCache, Resource},
        worker::logs::MockWorkerLogsDrivenStorage,
    };

    use super::*;

    impl Default for FetchCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                worker_id: Uuid::new_v4().to_string(),
                cursor: 1_717_000_000,
                limit: 100,
                direction: FetchDirection::Next,
            }
        }
    }

    #[tokio::test]
    async fn should_fetch_worker_logs() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut logs_storage = MockWorkerLogsDrivenStorage::new();
        logs_storage
            .expect_next()
            .return_once(|_, _, _| Ok(vec![Log::default()]));

        let cmd = FetchCmd::default();

        let result = fetch(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(logs_storage),
            cmd,
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn should_fail_to_fetch_worker_logs_without_user_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let logs_storage = MockWorkerLogsDrivenStorage::new();

        let cmd = FetchCmd::default();

        let result = fetch(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(logs_storage),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn should_fail_to_fetch_worker_logs_without_secret_permission() {
        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Resource::default())));

        let project_cache = MockProjectDrivenCache::new();
        let logs_storage = MockWorkerLogsDrivenStorage::new();

        let cmd = FetchCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = fetch(
            Arc::new(project_cache),
            Arc::new(resource_cache),
            Arc::new(logs_storage),
            cmd,
        )
        .await;
        assert!(result.is_err());
    }
}
