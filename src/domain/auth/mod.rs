use std::sync::Arc;

use serde::Deserialize;

use super::{
    error::Error,
    project::{cache::ProjectDrivenCache, ProjectUserRole},
};

use crate::domain::Result;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait Auth0Driven: Send + Sync {
    fn verify(&self, token: &str) -> Result<String>;
    async fn find_info(&self, user_id: &str) -> Result<Auth0Profile>;
    async fn find_info_by_ids(&self, ids: &[String]) -> Result<Vec<Auth0Profile>>;
}

pub type UserId = String;
pub type SecretId = String;

#[derive(Debug, Clone)]
pub enum Credential {
    Auth0(UserId),
    ApiKey(SecretId),
}

#[derive(Debug, Deserialize)]
pub struct Auth0Profile {
    pub user_id: String,
    pub name: String,
    pub email: String,
}

pub async fn assert_permission(
    project_cache: Arc<dyn ProjectDrivenCache>,
    credential: &Credential,
    project_id: &str,
    role: Option<ProjectUserRole>,
) -> Result<()> {
    match credential {
        Credential::Auth0(user_id) => {
            let result = project_cache
                .find_user_permission(user_id, project_id)
                .await?;

            if result.is_none() {
                return Err(Error::Unauthorized("user doesnt have permission".into()));
            }

            match role {
                Some(role) => {
                    let permission = result.unwrap();
                    if role != permission.role {
                        return Err(Error::Unauthorized("user doesnt have permission".into()));
                    }
                    Ok(())
                }
                None => Ok(()),
            }
        }
        Credential::ApiKey(secret_project_id) => {
            if project_id != secret_project_id {
                return Err(Error::Unauthorized("secret doesnt have permission".into()));
            }

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Default for Auth0Profile {
        fn default() -> Self {
            Self {
                user_id: "Auth0".into(),
                name: "user name".into(),
                email: "user email".into(),
            }
        }
    }
}
