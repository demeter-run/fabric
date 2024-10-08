use super::{auth::Auth0Driven, event::Event, Result};
use std::sync::Arc;

#[async_trait::async_trait]
pub trait NotifyDriven: Send + Sync {
    async fn notify(&self, evt: Event, auth_driven: Arc<dyn Auth0Driven>) -> Result<()>;
}
