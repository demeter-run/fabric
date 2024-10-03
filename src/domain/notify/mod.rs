use super::{event::Event, Result};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait NotifyDriven: Send + Sync {
    async fn notify(&self, evt: Event) -> Result<()>;
}
