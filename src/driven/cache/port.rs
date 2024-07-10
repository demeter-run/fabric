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
                INSERT INTO ports (id, project, kind, data)
                VALUES ($1, $2, $3, $4)
            "#,
            port.id,
            port.project,
            port.kind,
            port.data
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }
}
