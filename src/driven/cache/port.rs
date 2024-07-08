use anyhow::Result;
use std::sync::Arc;

use crate::domain::management::port::{Port, PortCache};

use super::SqliteCache;

pub struct SqlitePortCache {
    sqlite: Arc<SqliteCache>,
}
impl SqlitePortCache {
    pub fn new(sqlite: Arc<SqliteCache>) -> Self {
        Self { sqlite }
    }
}
#[async_trait::async_trait]
impl PortCache for SqlitePortCache {
    async fn create(&self, port: &Port) -> Result<()> {
        sqlx::query!(
            r#"
                INSERT INTO ports (project, kind)
                VALUES ($1, $2)
            "#,
            port.project,
            port.kind
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }
}
