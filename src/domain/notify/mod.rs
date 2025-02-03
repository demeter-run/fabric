use super::{
    auth::Auth0Driven, event::Event, project::cache::ProjectDrivenCache,
    resource::cache::ResourceDrivenCache, Result,
};
use std::sync::Arc;

#[async_trait::async_trait]
pub trait NotifyDriven: Send + Sync {
    async fn notify(
        &self,
        evt: Event,
        auth_driven: Arc<dyn Auth0Driven>,
        resource_cache: Arc<dyn ResourceDrivenCache>,
        project_cache: Arc<dyn ProjectDrivenCache>,
    ) -> Result<()>;
}
