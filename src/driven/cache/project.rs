use anyhow::Result;
use sqlx::{sqlite::SqliteRow, FromRow, Row};
use std::sync::Arc;

use crate::domain::project::{
    ProjectCache, ProjectDrivenCache, ProjectSecretCache, ProjectUserCache,
};

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
        let project = sqlx::query_as::<_, ProjectCache>(
            r#"
                SELECT id, namespace, name, owner 
                FROM project WHERE id = $1;
            "#,
        )
        .bind(namespace)
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(project)
    }
    async fn find_by_id(&self, id: &str) -> Result<Option<ProjectCache>> {
        let project = sqlx::query_as::<_, ProjectCache>(
            r#"
                SELECT id, namespace, name, owner 
                FROM project WHERE id = $1;
            "#,
        )
        .bind(id)
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(project)
    }

    async fn create(&self, project: &ProjectCache) -> Result<()> {
        let mut tx = self.sqlite.db.begin().await?;

        sqlx::query!(
            r#"
                INSERT INTO project (id, namespace, name, owner)
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
                INSERT INTO project_user (project_id, user_id)
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
    async fn create_secret(&self, secret: &ProjectSecretCache) -> Result<()> {
        sqlx::query!(
            r#"
                INSERT INTO project_secret (id, project_id, name, phc)
                VALUES ($1, $2, $3, $4)
            "#,
            secret.id,
            secret.project_id,
            secret.name,
            secret.phc,
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }
    async fn find_secret_by_project_id(&self, project_id: &str) -> Result<Vec<ProjectSecretCache>> {
        let secrets = sqlx::query_as::<_, ProjectSecretCache>(
            r#"
                SELECT id, project_id, name, phc 
                FROM project_secret WHERE project_id = $1;
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(secrets)
    }
    async fn find_user_permission(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Option<ProjectUserCache>> {
        let project_user = sqlx::query_as::<_, ProjectUserCache>(
            r#"
                SELECT user_id, project_id
                FROM project_user WHERE user_id = $1 and project_id = $2;
            "#,
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(project_user)
    }
}

impl FromRow<'_, SqliteRow> for ProjectCache {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            namespace: row.try_get("namespace")?,
            owner: row.try_get("owner")?,
        })
    }
}

impl FromRow<'_, SqliteRow> for ProjectSecretCache {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            name: row.try_get("name")?,
            phc: row.try_get("phc")?,
        })
    }
}

impl FromRow<'_, SqliteRow> for ProjectUserCache {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            user_id: row.try_get("user_id")?,
            project_id: row.try_get("project_id")?,
        })
    }
}
