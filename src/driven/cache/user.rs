use anyhow::Result;
use std::sync::Arc;

use crate::domain::management::user::{User, UserCache};

use super::SqliteCache;

pub struct SqliteUserCache {
    sqlite: Arc<SqliteCache>,
}
impl SqliteUserCache {
    pub fn new(sqlite: Arc<SqliteCache>) -> Self {
        Self { sqlite }
    }
}
#[async_trait::async_trait]
impl UserCache for SqliteUserCache {
    async fn create(&self, user: &User) -> Result<()> {
        sqlx::query!(
            r#"
                INSERT INTO users (id, email, auth_provider, auth_provider_id)
                VALUES ($1, $2, $3, $4)
            "#,
            user.id,
            user.email,
            user.auth_provider,
            user.auth_provider_id
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }
    async fn get_by_auth_provider_id(&self, id: &str) -> Result<Option<User>> {
        let result = sqlx::query!(
            r#"
                SELECT id, email, auth_provider, auth_provider_id
                FROM users WHERE auth_provider_id = $1;
            "#,
            id
        )
        .fetch_optional(&self.sqlite.db)
        .await?;

        if result.is_none() {
            return Ok(None);
        }

        let result = result.unwrap();

        let user = User {
            id: result.id,
            email: result.email,
            auth_provider: result.auth_provider,
            auth_provider_id: result.auth_provider_id,
        };

        Ok(Some(user))
    }
}
