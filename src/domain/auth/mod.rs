use std::sync::Arc;

use super::{error::Error, project::cache::ProjectDrivenCache};

use crate::domain::Result;

#[async_trait::async_trait]
pub trait Auth0Driven: Send + Sync {
    fn verify(&self, token: &str) -> Result<String>;
    async fn find_info(&self, user_id: &str) -> Result<(String, String)>;
}

pub type UserId = String;
pub type SecretId = String;

#[derive(Debug, Clone)]
pub enum Credential {
    Auth0(UserId),
    ApiKey(SecretId),
}

pub async fn assert_project_permission(
    project_cache: Arc<dyn ProjectDrivenCache>,
    credential: &Credential,
    project_id: &str,
) -> Result<()> {
    match credential {
        Credential::Auth0(user_id) => {
            let result = project_cache
                .find_user_permission(user_id, project_id)
                .await?;

            if result.is_none() {
                return Err(Error::Unauthorized("user doesnt have permission".into()));
            }

            Ok(())
        }
        Credential::ApiKey(secret_project_id) => {
            if project_id != secret_project_id {
                return Err(Error::Unauthorized("secret doesnt have permission".into()));
            }

            Ok(())
        }
    }
}
