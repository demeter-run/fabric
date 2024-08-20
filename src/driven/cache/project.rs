use sqlx::{sqlite::SqliteRow, FromRow, Row};
use std::sync::Arc;

use crate::domain::{
    error::Error,
    project::{
        cache::ProjectDrivenCache, Project, ProjectSecret, ProjectStatus, ProjectUpdate,
        ProjectUser,
    },
    Result,
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
    async fn find(&self, user_id: &str, page: &u32, page_size: &u32) -> Result<Vec<Project>> {
        let offset = page_size * (page - 1);

        let projects = sqlx::query_as::<_, Project>(
            r#"
                SELECT 
                    p.id, 
                    p.namespace, 
                    p.name, 
                    p.owner, 
                    p.status, 
                    p.created_at, 
                    p.updated_at
                FROM project_user pu 
                INNER JOIN project p on p.id = pu.project_id
                WHERE pu.user_id = $1 and p.status != $2
                ORDER BY pu.created_at DESC
                LIMIT $3
                OFFSET $4;
            "#,
        )
        .bind(user_id)
        .bind(ProjectStatus::Deleted.to_string())
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(projects)
    }
    async fn find_by_namespace(&self, namespace: &str) -> Result<Option<Project>> {
        let project = sqlx::query_as::<_, Project>(
            r#"
                SELECT 
                    p.id, 
                    p.namespace, 
                    p.name, 
                    p.owner, 
                    p.status, 
                    p.created_at, 
                    p.updated_at
                FROM project p
                WHERE p.namespace = $1 and p.status != $2;
            "#,
        )
        .bind(namespace)
        .bind(ProjectStatus::Deleted.to_string())
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(project)
    }
    async fn find_by_id(&self, id: &str) -> Result<Option<Project>> {
        let project = sqlx::query_as::<_, Project>(
            r#"
                SELECT 
                    p.id, 
                    p.namespace, 
                    p.name, 
                    p.owner, 
                    p.status, 
                    p.created_at, 
                    p.updated_at
                FROM project p 
                WHERE p.id = $1 and p.status != $2;
            "#,
        )
        .bind(id)
        .bind(ProjectStatus::Deleted.to_string())
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(project)
    }

    async fn create(&self, project: &Project) -> Result<()> {
        let mut tx = self.sqlite.db.begin().await?;

        let status = project.status.to_string();

        sqlx::query!(
            r#"
                INSERT INTO project (
                    id,
                    namespace,
                    name,
                    owner,
                    status,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            project.id,
            project.namespace,
            project.name,
            project.owner,
            status,
            project.created_at,
            project.updated_at
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            r#"
                INSERT INTO project_user (
                    project_id,
                    user_id,
                    created_at
                )
                VALUES ($1, $2, $3)
            "#,
            project.id,
            project.owner,
            project.created_at
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    async fn update(&self, project_update: &ProjectUpdate) -> Result<()> {
        match (&project_update.name, &project_update.status) {
            (Some(new_name), Some(new_status)) => {
                let new_status = new_status.to_string();
                sqlx::query!(
                    r#"
                        UPDATE project
                        SET name = $1, status = $2, updated_at = $3
                        WHERE id = $4
                    "#,
                    new_name,
                    new_status,
                    project_update.updated_at,
                    project_update.id,
                )
                .execute(&self.sqlite.db)
                .await?;

                Ok(())
            }
            (Some(new_name), None) => {
                sqlx::query!(
                    r#"
                        UPDATE project
                        SET name = $1, updated_at = $2
                        WHERE id = $3
                    "#,
                    new_name,
                    project_update.updated_at,
                    project_update.id,
                )
                .execute(&self.sqlite.db)
                .await?;

                Ok(())
            }
            (None, Some(new_status)) => {
                let new_status = new_status.to_string();
                sqlx::query!(
                    r#"
                        UPDATE project
                        SET status = $1, updated_at = $2
                        WHERE id = $3
                    "#,
                    new_status,
                    project_update.updated_at,
                    project_update.id,
                )
                .execute(&self.sqlite.db)
                .await?;

                Ok(())
            }
            (None, None) => Ok(()),
        }
    }
    async fn create_secret(&self, secret: &ProjectSecret) -> Result<()> {
        sqlx::query!(
            r#"
                INSERT INTO project_secret (
                    id,
                    project_id, 
                    name, 
                    phc, 
                    secret,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            secret.id,
            secret.project_id,
            secret.name,
            secret.phc,
            secret.secret,
            secret.created_at
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }
    async fn find_secret_by_project_id(&self, project_id: &str) -> Result<Vec<ProjectSecret>> {
        let secrets = sqlx::query_as::<_, ProjectSecret>(
            r#"
                SELECT 
                    ps.id, 
                    ps.project_id, 
                    ps.name, 
                    ps.phc, 
                    ps.secret, 
                    ps.created_at
                FROM project_secret ps 
                WHERE ps.project_id = $1;
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
    ) -> Result<Option<ProjectUser>> {
        let project_user = sqlx::query_as::<_, ProjectUser>(
            r#"
                SELECT 
                    pu.user_id, 
                    pu.project_id, 
                    pu.created_at
                FROM project_user pu 
                WHERE pu.user_id = $1 and pu.project_id = $2;
            "#,
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(project_user)
    }
}

impl FromRow<'_, SqliteRow> for Project {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        let status: &str = row.try_get("status")?;

        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            namespace: row.try_get("namespace")?,
            owner: row.try_get("owner")?,
            status: status
                .parse()
                .map_err(|err: Error| sqlx::Error::Decode(err.into()))?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl FromRow<'_, SqliteRow> for ProjectSecret {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            name: row.try_get("name")?,
            phc: row.try_get("phc")?,
            secret: row.try_get("secret")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl FromRow<'_, SqliteRow> for ProjectUser {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            user_id: row.try_get("user_id")?,
            project_id: row.try_get("project_id")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn get_cache() -> SqliteProjectDrivenCache {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        SqliteProjectDrivenCache::new(sqlite_cache)
    }

    #[tokio::test]
    async fn it_should_create_project() {
        let cache = get_cache().await;
        let project = Project::default();

        let result = cache.create(&project).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_find_user_projects() {
        let cache = get_cache().await;
        let project = Project::default();

        cache.create(&project).await.unwrap();
        let result = cache.find(&project.owner, &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().len() == 1);
    }
    #[tokio::test]
    async fn it_should_return_none_find_user_projects_invalid_page() {
        let cache = get_cache().await;
        let project = Project::default();

        cache.create(&project).await.unwrap();
        let result = cache.find(&project.owner, &2, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
    #[tokio::test]
    async fn it_should_return_none_find_user_projects() {
        let cache = get_cache().await;
        let result = cache.find(Default::default(), &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_should_find_project_by_id() {
        let cache = get_cache().await;
        let project = Project::default();

        cache.create(&project).await.unwrap();
        let result = cache.find_by_id(&project.id).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
    #[tokio::test]
    async fn it_should_return_none_find_project_by_id() {
        let cache = get_cache().await;
        let project = Project::default();

        let result = cache.find_by_id(&project.id).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_should_find_project_by_namespace() {
        let cache = get_cache().await;
        let project = Project::default();

        cache.create(&project).await.unwrap();
        let result = cache.find_by_namespace(&project.namespace).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
    #[tokio::test]
    async fn it_should_return_none_find_project_by_namespace() {
        let cache = get_cache().await;
        let project = Project::default();

        let result = cache.find_by_namespace(&project.namespace).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_should_create_project_secret() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let secret = ProjectSecret {
            project_id: project.id,
            ..Default::default()
        };
        let result = cache.create_secret(&secret).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_find_secret_by_project_id() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let secret = ProjectSecret {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create_secret(&secret).await.unwrap();

        let result = cache.find_secret_by_project_id(&project.id).await;

        assert!(result.is_ok());
        assert!(result.unwrap().len() == 1);
    }

    #[tokio::test]
    async fn it_should_find_user_permission() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let result = cache
            .find_user_permission(&project.owner, &project.id)
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
    #[tokio::test]
    async fn it_should_return_none_find_user_permission() {
        let cache = get_cache().await;

        let project = Project::default();

        let result = cache
            .find_user_permission(&project.owner, &project.id)
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
