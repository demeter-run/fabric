use anyhow::Result;
use std::sync::Arc;

use crate::domain::project::{ProjectCache, ProjectDrivenCache};

use super::SqliteCache;

pub struct SqliteProjectDrivenCache {
    sqlite: Arc<SqliteCache>,
}
impl SqliteProjectDrivenCache {
    pub fn new(sqlite: Arc<SqliteCache>) -> Self {
        Self { sqlite }
    }
}
#[async_trait::async_trait]
impl ProjectDrivenCache for SqliteProjectDrivenCache {
    async fn find_by_namespace(&self, namespace: &str) -> Result<Option<ProjectCache>> {
        let result = sqlx::query!(
            r#"
                SELECT id, namespace, name, owner 
                FROM projects WHERE id = $1;
            "#,
            namespace
        )
        .fetch_optional(&self.sqlite.db)
        .await?;

        if result.is_none() {
            return Ok(None);
        }

        let result = result.unwrap();

        let project = ProjectCache {
            id: result.id,
            namespace: result.namespace,
            name: result.name,
            owner: result.owner,
        };

        Ok(Some(project))
    }
    async fn find_by_id(&self, id: &str) -> Result<Option<ProjectCache>> {
        let result = sqlx::query!(
            r#"
                SELECT id, namespace, name, owner 
                FROM projects WHERE id = $1;
            "#,
            id
        )
        .fetch_optional(&self.sqlite.db)
        .await?;

        if result.is_none() {
            return Ok(None);
        }

        let result = result.unwrap();

        let project = ProjectCache {
            id: result.id,
            namespace: result.namespace,
            name: result.name,
            owner: result.owner,
        };

        Ok(Some(project))
    }

    async fn create(&self, project: &ProjectCache) -> Result<()> {
        let mut tx = self.sqlite.db.begin().await?;

        sqlx::query!(
            r#"
                INSERT INTO projects (id, namespace, name, owner)
                VALUES ($1, $2, $3, $4)
            "#,
            project.id,
            project.namespace,
            project.name,
            project.owner,
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            r#"
                INSERT INTO projects_users (project_id, user_id)
                VALUES ($1, $2)
            "#,
            project.id,
            project.owner,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }
}
