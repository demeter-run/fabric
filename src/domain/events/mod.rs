use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{ports::Port, projects::Project, users::User};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::enum_variant_names)]
pub enum Event {
    ProjectCreated(Project),
    UserCreated(User),
    PortCreated(Port),
}

#[async_trait::async_trait]
pub trait EventBridge: Send + Sync {
    async fn dispatch(&self, event: Event) -> Result<()>;
}
