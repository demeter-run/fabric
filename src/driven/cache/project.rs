use anyhow::Result;
use std::sync::Arc;

use crate::domain::projects::{Project, ProjectCache, ProjectUser};

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
        let mut tx = self.sqlite.db.begin().await?;

        sqlx::query!(
            r#"
                INSERT INTO projects (id, namespace, name, created_by)
                VALUES ($1, $2, $3, $4)
            "#,
            project.id,
            project.namespace,
            project.name,
            project.created_by,
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            r#"
                INSERT INTO projects_users (project_id, user_id)
                VALUES ($1, $2)
            "#,
            project.id,
            project.created_by,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }
    async fn find_by_id(&self, id: &str) -> Result<Option<Project>> {
        let result = sqlx::query!(
            r#"
                SELECT id, namespace, name, created_by 
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

        let project = Project {
            id: result.id,
            namespace: result.namespace,
            name: result.name,
            created_by: result.created_by,
        };

        Ok(Some(project))
    }
    async fn find_user_permission(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Option<ProjectUser>> {
        let result = sqlx::query!(
            r#"
                SELECT user_id, project_id 
                FROM projects_users WHERE user_id = $1 and project_id = $2;
            "#,
            user_id,
            project_id
        )
        .fetch_optional(&self.sqlite.db)
        .await?;

        if result.is_none() {
            return Ok(None);
        }

        let result = result.unwrap();

        let project_user = ProjectUser {
            user_id: result.user_id,
            project_id: result.project_id,
        };

        Ok(Some(project_user))
    }
}
