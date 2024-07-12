use anyhow::{bail, Result};
use std::sync::Arc;
use tracing::{error, info};

use crate::domain::events::{Event, EventBridge};

use super::{AuthProvider, User, UserCache};

pub async fn create(
    cache: Arc<dyn UserCache>,
    auth: Arc<dyn AuthProvider>,
    event: Arc<dyn EventBridge>,
    token: String,
) -> Result<User> {
    let verify_result = auth.verify(&token).await;
    if let Err(err) = verify_result {
        error!(error = err.to_string(), "invalid access token");
        bail!("invalid access token");
    }

    let auth_provider_id = verify_result.unwrap();
    if let Some(user) = cache.get_by_auth_provider_id(&auth_provider_id).await? {
        return Ok(user);
    }

    let email_result = auth.get_profile(&token).await;
    if let Err(err) = email_result {
        error!(error = err.to_string(), "error to get user info");
        bail!("invalid access token");
    }
    let email = email_result.unwrap();

    let user = User::new(email, auth_provider_id);
    let user_event = Event::UserCreated(user.clone());

    event.dispatch(user_event).await?;
    info!(user = user.id, "new user created");

    Ok(user)
}

pub async fn create_cache(cache: Arc<dyn UserCache>, user: User) -> Result<()> {
    if let Some(user) = cache
        .get_by_auth_provider_id(&user.auth_provider_id)
        .await?
    {
        info!(user = user.id, "user already exists");
        return Ok(());
    }

    cache.create(&user).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use mockall::mock;
    use uuid::Uuid;

    use crate::domain::users::AUTH_PROVIDER;

    use super::*;

    mock! {
        pub FakeUserCache { }

        #[async_trait::async_trait]
        impl UserCache for FakeUserCache {
            async fn create(&self, user: &User) -> Result<()>;
            async fn get_by_auth_provider_id(&self, id: &str) -> Result<Option<User>>;
        }
    }

    mock! {
        pub FakeAuthProvider { }

        #[async_trait::async_trait]
        impl AuthProvider for FakeAuthProvider {
            async fn verify(&self, token: &str) -> Result<String>;
            async fn get_profile(&self, token: &str) -> Result<String>;
        }
    }

    mock! {
        pub FakeEventBridge { }

        #[async_trait::async_trait]
        impl EventBridge for FakeEventBridge {
            async fn dispatch(&self, event: Event) -> Result<()>;
        }
    }

    impl Default for User {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().into(),
                email: "cw@txpipe.io".into(),
                auth_provider: AUTH_PROVIDER.into(),
                auth_provider_id: "google-oauth2|xxx".into(),
            }
        }
    }

    #[tokio::test]
    async fn it_should_create_user() {
        let mut auth_provider = MockFakeAuthProvider::new();
        auth_provider
            .expect_verify()
            .return_once(|_| Ok("google-oauth2|xxx".into()));
        auth_provider
            .expect_get_profile()
            .return_once(|_| Ok("cw@txpipe.io".into()));

        let mut user_cache = MockFakeUserCache::new();
        user_cache
            .expect_get_by_auth_provider_id()
            .return_once(|_| Ok(None));
        user_cache.expect_create().return_once(|_| Ok(()));

        let mut event_bridge = MockFakeEventBridge::new();
        event_bridge.expect_dispatch().return_once(|_| Ok(()));

        let result = create(
            Arc::new(user_cache),
            Arc::new(auth_provider),
            Arc::new(event_bridge),
            Default::default(),
        )
        .await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
    #[tokio::test]
    async fn it_should_return_user_existing() {
        let mut auth_provider = MockFakeAuthProvider::new();
        auth_provider
            .expect_verify()
            .return_once(|_| Ok("google-oauth2|xxx".into()));

        let mut user_cache = MockFakeUserCache::new();
        user_cache
            .expect_get_by_auth_provider_id()
            .return_once(|_| Ok(Some(User::default())));

        let event_bridge = MockFakeEventBridge::new();

        let result = create(
            Arc::new(user_cache),
            Arc::new(auth_provider),
            Arc::new(event_bridge),
            Default::default(),
        )
        .await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
    #[tokio::test]
    async fn it_should_fail_when_invalid_token() {
        let mut auth_provider = MockFakeAuthProvider::new();
        auth_provider
            .expect_verify()
            .return_once(|_| bail!("invalid token"));

        let user_cache = MockFakeUserCache::new();
        let event_bridge = MockFakeEventBridge::new();

        let result = create(
            Arc::new(user_cache),
            Arc::new(auth_provider),
            Arc::new(event_bridge),
            Default::default(),
        )
        .await;
        if result.is_ok() {
            unreachable!("it should fail when invalid token")
        }
    }

    #[tokio::test]
    async fn it_should_create_user_cache() {
        let mut user_cache = MockFakeUserCache::new();
        user_cache
            .expect_get_by_auth_provider_id()
            .return_once(|_| Ok(None));
        user_cache.expect_create().return_once(|_| Ok(()));

        let user = User::default();

        let result = create_cache(Arc::new(user_cache), user).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
    #[tokio::test]
    async fn it_should_ignore_create_user_cache() {
        let mut user_cache = MockFakeUserCache::new();
        user_cache
            .expect_get_by_auth_provider_id()
            .return_once(|_| Ok(Some(User::default())));

        let user = User::default();

        let result = create_cache(Arc::new(user_cache), user).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
}
