use anyhow::Result;
use std::sync::Arc;

use crate::domain::management::project::{Project, ProjectCache};

use super::SqliteCache;

pub struct SqliteProjectCache {
    sqlite: Arc<SqliteCache>,
}
impl SqliteProjectCache {
    pub fn new(sqlite: Arc<SqliteCache>) -> Self {
        Self { sqlite }
    }
}
#[async_trait::async_trait]
impl ProjectCache for SqliteProjectCache {
    async fn create(&self, project: &Project) -> Result<()> {
        sqlx::query!(
            r#"
                INSERT INTO projects (slug, name)
                VALUES ($1, $2)
            "#,
            project.slug,
            project.name,
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }
    async fn find_by_slug(&self, slug: &str) -> Result<Option<Project>> {
        let result = sqlx::query!(
            r#"
                SELECT slug, name 
                FROM projects WHERE slug = $1;
            "#,
            slug
        )
        .fetch_optional(&self.sqlite.db)
        .await?;

        if result.is_none() {
            return Ok(None);
        }

        Ok(None)
    }
}
