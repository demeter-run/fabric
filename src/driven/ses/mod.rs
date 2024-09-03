use chrono::{DateTime, Utc};

use crate::domain::{project::ProjectEmailDriven, Result};

pub struct SESDrivenImpl {}
impl SESDrivenImpl {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl ProjectEmailDriven for SESDrivenImpl {
    async fn send_invite(
        &self,
        _email: &str,
        _code: &str,
        _expire_in: &DateTime<Utc>,
    ) -> Result<()> {
        todo!()
    }
}
