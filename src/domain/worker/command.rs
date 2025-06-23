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
