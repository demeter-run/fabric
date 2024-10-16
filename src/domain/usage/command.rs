use std::sync::Arc;

use crate::domain::{
    auth::{assert_permission, Credential},
    error::Error,
    metadata::MetadataDriven,
    project::cache::ProjectDrivenCache,
    Result, PAGE_SIZE_DEFAULT, PAGE_SIZE_MAX,
};

use super::{cache::UsageDrivenCache, UsageReport, UsageReportImpl};

pub async fn fetch_report(
    project_cache: Arc<dyn ProjectDrivenCache>,
    usage_cache: Arc<dyn UsageDrivenCache>,
    metadata: Arc<dyn MetadataDriven>,
    cmd: FetchCmd,
) -> Result<Vec<UsageReport>> {
    assert_permission(
        project_cache.clone(),
        &cmd.credential,
        &cmd.project_id,
        None,
    )
    .await?;

    let usage = usage_cache
        .find_report(&cmd.project_id, &cmd.page, &cmd.page_size)
        .await?
        .calculate_cost(metadata.clone());

    Ok(usage)
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

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::{
        metadata::{MockMetadataDriven, ResourceMetadata},
        project::{cache::MockProjectDrivenCache, ProjectUser},
        usage::cache::MockUsageDrivenCache,
    };

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

    #[tokio::test]
    async fn it_should_fetch_project_usage_report() {
        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut usage_cache = MockUsageDrivenCache::new();
        usage_cache
            .expect_find_report()
            .return_once(|_, _, _| Ok(vec![UsageReport::default()]));

        let mut metadata = MockMetadataDriven::new();
        metadata
            .expect_find_by_kind()
            .return_once(|_| Ok(Some(ResourceMetadata::default())));

        let cmd = FetchCmd::default();

        let result = fetch_report(
            Arc::new(project_cache),
            Arc::new(usage_cache),
            Arc::new(metadata),
            cmd,
        )
        .await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_fetch_project_usage_report_when_user_doesnt_have_permission() {
        let mut project_cache = MockProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let usage_cache = MockUsageDrivenCache::new();

        let cmd = FetchCmd::default();

        let metadata = MockMetadataDriven::new();

        let result = fetch_report(
            Arc::new(project_cache),
            Arc::new(usage_cache),
            Arc::new(metadata),
            cmd,
        )
        .await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_fetch_project_usage_report_when_secret_doesnt_have_permission() {
        let project_cache = MockProjectDrivenCache::new();
        let usage_cache = MockUsageDrivenCache::new();

        let cmd = FetchCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let metadata = MockMetadataDriven::new();

        let result = fetch_report(
            Arc::new(project_cache),
            Arc::new(usage_cache),
            Arc::new(metadata),
            cmd,
        )
        .await;
        assert!(result.is_err());
    }
}
