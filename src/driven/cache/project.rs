use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqliteRow, FromRow, Row};
use std::sync::Arc;

use crate::domain::{
    error::Error,
    project::{
        cache::ProjectDrivenCache, Project, ProjectSecret, ProjectStatus, ProjectUpdate,
        ProjectUser, ProjectUserInvite, ProjectUserInviteStatus, ProjectUserRole,
    },
    resource::ResourceStatus,
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
                    p.billing_provider,
                    p.billing_provider_id,
                    p.billing_subscription_id,
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
                    p.billing_provider,
                    p.billing_provider_id,
                    p.billing_subscription_id,
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
                    p.billing_provider,
                    p.billing_provider_id,
                    p.billing_subscription_id,
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
                    billing_provider,
                    billing_provider_id,
                    billing_subscription_id,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            project.id,
            project.namespace,
            project.name,
            project.owner,
            status,
            project.billing_provider,
            project.billing_provider_id,
            project.billing_subscription_id,
            project.created_at,
            project.updated_at
        )
        .execute(&mut *tx)
        .await?;

        let role = ProjectUserRole::Owner.to_string();
        sqlx::query!(
            r#"
                INSERT INTO project_user (
                    project_id,
                    user_id,
                    role,
                    created_at
                )
                VALUES ($1, $2, $3, $4)
            "#,
            project.id,
            project.owner,
            role,
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

    async fn delete(&self, id: &str, deleted_at: &DateTime<Utc>) -> Result<()> {
        let status = ProjectStatus::Deleted.to_string();

        let mut tx = self.sqlite.db.begin().await?;
        sqlx::query!(
            r#"
                UPDATE project
                SET status=$2, updated_at=$3
                WHERE id=$1;
            "#,
            id,
            status,
            deleted_at
        )
        .execute(&mut *tx)
        .await?;

        let status = ResourceStatus::Deleted.to_string();
        sqlx::query!(
            r#"
                UPDATE resource
                SET status=$2, updated_at=$3
                WHERE project_id=$1;
            "#,
            id,
            status,
            deleted_at
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
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
    async fn find_secrets(&self, project_id: &str) -> Result<Vec<ProjectSecret>> {
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
                WHERE ps.project_id = $1
                ORDER BY ps.created_at DESC;
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(secrets)
    }
    async fn find_secret_by_id(&self, id: &str) -> Result<Option<ProjectSecret>> {
        let secret = sqlx::query_as::<_, ProjectSecret>(
            r#"
                SELECT 
                    ps.id, 
                    ps.project_id, 
                    ps.name, 
                    ps.phc, 
                    ps.secret, 
                    ps.created_at
                FROM project_secret ps 
                WHERE ps.id = $1;
            "#,
        )
        .bind(id)
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(secret)
    }
    async fn delete_secret(&self, id: &str) -> Result<()> {
        sqlx::query!(
            r#"
                DELETE FROM
                    project_secret
                WHERE 
                    id=$1;
            "#,
            id,
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
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
                    pu.role, 
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

    async fn find_users(
        &self,
        project_id: &str,
        page: &u32,
        page_size: &u32,
    ) -> Result<Vec<ProjectUser>> {
        let offset = page_size * (page - 1);

        let users = sqlx::query_as::<_, ProjectUser>(
            r#"

                SELECT 
                    pu.user_id, 
                    pu.project_id, 
                    pu.role, 
                    pu.created_at
                FROM project_user pu 
                WHERE pu.project_id = $1
                ORDER BY pu.created_at DESC
                LIMIT $2
                OFFSET $3;
            "#,
        )
        .bind(project_id)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(users)
    }

    async fn find_user_invites(
        &self,
        project_id: &str,
        page: &u32,
        page_size: &u32,
    ) -> Result<Vec<ProjectUserInvite>> {
        let offset = page_size * (page - 1);

        let invites = sqlx::query_as::<_, ProjectUserInvite>(
            r#"
                SELECT 
                    pui.id,
                    pui.project_id,
                    pui.email,
                    pui."role",
                    pui.code,
                    pui.status,
                    pui.expires_in,
                    pui.created_at,
                    pui.updated_at
                FROM project_user_invite pui
                WHERE pui.project_id = $1
                ORDER BY pui.created_at DESC
                LIMIT $2
                OFFSET $3;
            "#,
        )
        .bind(project_id)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(invites)
    }
    async fn find_user_invite_by_id(&self, id: &str) -> Result<Option<ProjectUserInvite>> {
        let invite = sqlx::query_as::<_, ProjectUserInvite>(
            r#"
                SELECT 
                    pui.id,
                    pui.project_id,
                    pui.email,
                    pui."role",
                    pui.code,
                    pui.status,
                    pui.expires_in,
                    pui.created_at,
                    pui.updated_at
                FROM project_user_invite pui
                WHERE pui.id = $1;
            "#,
        )
        .bind(id)
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(invite)
    }
    async fn find_user_invite_by_code(&self, code: &str) -> Result<Option<ProjectUserInvite>> {
        let invite = sqlx::query_as::<_, ProjectUserInvite>(
            r#"
                SELECT 
                    pui.id,
                    pui.project_id,
                    pui.email,
                    pui."role",
                    pui.code,
                    pui.status,
                    pui.expires_in,
                    pui.created_at,
                    pui.updated_at
                FROM project_user_invite pui
                WHERE pui.code = $1;
            "#,
        )
        .bind(code)
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(invite)
    }

    async fn create_user_invite(&self, invite: &ProjectUserInvite) -> Result<()> {
        let role = invite.role.to_string();
        let status = invite.status.to_string();

        sqlx::query!(
            r#"
                INSERT INTO project_user_invite(
                    id, 
                    project_id,
                    email,
                    "role",
                    code,
                    status,
                    expires_in, 
                    created_at,
                    updated_at 
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9);
            "#,
            invite.id,
            invite.project_id,
            invite.email,
            role,
            invite.code,
            status,
            invite.expires_in,
            invite.created_at,
            invite.updated_at
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }

    async fn create_user_acceptance(&self, invite_id: &str, user: &ProjectUser) -> Result<()> {
        let mut tx = self.sqlite.db.begin().await?;

        let role = user.role.to_string();
        sqlx::query!(
            r#"
                INSERT INTO project_user (
                    project_id,
                    user_id,
                    role,
                    created_at
                )
                VALUES ($1, $2, $3, $4)
            "#,
            user.project_id,
            user.user_id,
            role,
            user.created_at
        )
        .execute(&mut *tx)
        .await?;

        let status = ProjectUserInviteStatus::Accepted.to_string();
        sqlx::query!(
            r#"
                UPDATE project_user_invite 
                SET status = $1, updated_at = $2
                WHERE id = $3
            "#,
            status,
            user.created_at,
            invite_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    async fn delete_user(&self, project_id: &str, id: &str) -> Result<()> {
        sqlx::query!(
            r#"
                DELETE FROM
                    project_user
                WHERE 
                    project_id=$1 AND user_id=$2;
            "#,
            project_id,
            id,
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
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
            billing_provider: row.try_get("billing_provider")?,
            billing_provider_id: row.try_get("billing_provider_id")?,
            billing_subscription_id: row.try_get("billing_subscription_id")?,
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
        let role: &str = row.try_get("role")?;

        Ok(Self {
            user_id: row.try_get("user_id")?,
            project_id: row.try_get("project_id")?,
            role: role
                .parse()
                .map_err(|err: Error| sqlx::Error::Decode(err.into()))?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl FromRow<'_, SqliteRow> for ProjectUserInvite {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        let role: &str = row.try_get("role")?;
        let status: &str = row.try_get("status")?;

        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            email: row.try_get("email")?,
            code: row.try_get("code")?,
            role: role
                .parse()
                .map_err(|err: Error| sqlx::Error::Decode(err.into()))?,
            status: status
                .parse()
                .map_err(|err: Error| sqlx::Error::Decode(err.into()))?,
            expires_in: row.try_get("expires_in")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
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

        let result = cache.find_secrets(&project.id).await;

        assert!(result.is_ok());
        assert!(result.unwrap().len() == 1);
    }

    #[tokio::test]
    async fn it_should_find_secret_by_id() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let secret = ProjectSecret {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create_secret(&secret).await.unwrap();

        let result = cache.find_secret_by_id(&secret.id).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
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
    #[tokio::test]
    async fn it_should_find_user_invite_by_code() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let invite = ProjectUserInvite {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create_user_invite(&invite).await.unwrap();

        let result = cache.find_user_invite_by_code(&invite.code).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
    #[tokio::test]
    async fn it_should_return_none_find_user_invite_by_code() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let invite = ProjectUserInvite {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create_user_invite(&invite).await.unwrap();

        let result = cache.find_user_invite_by_code("invalid").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
    #[tokio::test]
    async fn it_should_find_user_invite_by_id() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let invite = ProjectUserInvite {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create_user_invite(&invite).await.unwrap();

        let result = cache.find_user_invite_by_id(&invite.id).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
    #[tokio::test]
    async fn it_should_return_none_find_user_invite_by_id() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let invite = ProjectUserInvite {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create_user_invite(&invite).await.unwrap();

        let result = cache.find_user_invite_by_id("invalid").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_should_find_user_invites() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let invite = ProjectUserInvite {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create_user_invite(&invite).await.unwrap();

        let result = cache.find_user_invites(&project.id, &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().len() == 1);
    }
    #[tokio::test]
    async fn it_should_return_none_find_user_invites() {
        let cache = get_cache().await;
        let result = cache.find_user_invites(Default::default(), &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_should_create_user_invite() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let invite = ProjectUserInvite {
            project_id: project.id,
            ..Default::default()
        };
        let result = cache.create_user_invite(&invite).await;

        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_create_user_acceptance() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let invite = ProjectUserInvite {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create_user_invite(&invite).await.unwrap();

        let project_user = ProjectUser {
            project_id: project.id,
            ..Default::default()
        };
        let result = cache
            .create_user_acceptance(&invite.id, &project_user)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_find_users() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let result = cache.find_users(&project.id, &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().len() == 1);
    }
    #[tokio::test]
    async fn it_should_return_none_find_users() {
        let cache = get_cache().await;
        let result = cache.find_users(Default::default(), &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_should_delete_user() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let invite = ProjectUserInvite {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create_user_invite(&invite).await.unwrap();

        let project_user = ProjectUser {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache
            .create_user_acceptance(&invite.id, &project_user)
            .await
            .unwrap();

        let result = cache.delete_user(&project.id, &project_user.user_id).await;

        assert!(result.is_ok());
    }
}
