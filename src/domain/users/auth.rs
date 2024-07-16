use anyhow::{bail, Result};
use std::sync::Arc;
use tracing::error;

use super::AuthProvider;

pub async fn validate(auth: Arc<dyn AuthProvider>, token: String) -> Result<String> {
    let verify_result = auth.verify(&token).await;
    if let Err(err) = verify_result {
        error!(error = err.to_string(), "invalid access token");
        bail!("invalid access token");
    }

    let user_id = verify_result.unwrap();
    Ok(user_id)
}

#[cfg(test)]
mod tests {
    use mockall::mock;

    use super::*;

    mock! {
        pub FakeAuthProvider { }

        #[async_trait::async_trait]
        impl AuthProvider for FakeAuthProvider {
            async fn verify(&self, token: &str) -> Result<String>;
        }
    }

    #[tokio::test]
    async fn it_should_validate_token() {
        let mut auth_provider = MockFakeAuthProvider::new();
        auth_provider
            .expect_verify()
            .return_once(|_| Ok("google-oauth2|xxx".into()));

        let result = validate(Arc::new(auth_provider), Default::default()).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
    #[tokio::test]
    async fn it_should_fail_when_token_is_invalid() {
        let mut auth_provider = MockFakeAuthProvider::new();
        auth_provider
            .expect_verify()
            .return_once(|_| bail!("invalid token"));

        let result = validate(Arc::new(auth_provider), Default::default()).await;
        if result.is_ok() {
            unreachable!("it should fail when invalid token")
        }
    }
}
