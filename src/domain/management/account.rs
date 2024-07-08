use anyhow::Result;
use std::sync::Arc;
use tracing::info;

use crate::domain::events::{AccountCreated, Event, EventBridge};

pub async fn create(
    _cache: Arc<dyn AccountCache>,
    event: Arc<dyn EventBridge>,
    account: Account,
) -> Result<()> {
    let account_event = Event::AccountCreated(account.clone().into());

    event.dispatch(account_event).await?;
    info!(account = account.name, "new account created");

    Ok(())
}

//TODO: remove later
#[allow(dead_code)]
pub async fn create_cache(cache: Arc<dyn AccountCache>, account: AccountCreated) -> Result<()> {
    cache.create(&account.into()).await?;

    Ok(())
}

#[derive(Debug, Clone)]
pub struct Account {
    pub name: String,
}
impl Account {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}
impl From<AccountCreated> for Account {
    fn from(value: AccountCreated) -> Self {
        Self { name: value.name }
    }
}
impl From<Account> for AccountCreated {
    fn from(value: Account) -> Self {
        AccountCreated { name: value.name }
    }
}

#[async_trait::async_trait]
pub trait AccountCache: Send + Sync {
    async fn create(&self, account: &Account) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use mockall::mock;

    use super::*;

    mock! {
        pub FakeAccountCache { }

        #[async_trait::async_trait]
        impl AccountCache for FakeAccountCache {
            async fn create(&self, account: &Account) -> Result<()>;
        }
    }

    mock! {
        pub FakeEventBridge { }

        #[async_trait::async_trait]
        impl EventBridge for FakeEventBridge {
            async fn dispatch(&self, event: Event) -> Result<()>;
        }
    }

    impl Default for Account {
        fn default() -> Self {
            Self {
                name: "New Account".into(),
            }
        }
    }

    #[tokio::test]
    async fn it_should_create_account() {
        let account_cache = MockFakeAccountCache::new();
        let mut event_bridge = MockFakeEventBridge::new();
        event_bridge.expect_dispatch().return_once(|_| Ok(()));

        let account = Account::default();

        let result = create(Arc::new(account_cache), Arc::new(event_bridge), account).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }

    #[tokio::test]
    async fn it_should_create_account_cache() {
        let mut account_cache = MockFakeAccountCache::new();
        account_cache.expect_create().return_once(|_| Ok(()));

        let account = Account::default();

        let result = create_cache(Arc::new(account_cache), account.into()).await;
        if let Err(err) = result {
            unreachable!("{err}")
        }
    }
}
