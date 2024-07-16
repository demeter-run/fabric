use anyhow::Result;
use std::sync::Arc;

use crate::domain::ports::{Port, PortCache};

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
                INSERT INTO ports (id, project_id, kind, data, created_by)
                VALUES ($1, $2, $3, $4, $5)
            "#,
            port.id,
            port.project_id,
            port.kind,
            port.data,
            port.created_by
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }
}
