use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqliteRow, FromRow, Row};
use std::sync::Arc;

use crate::domain::{
    error::Error,
    project::{
        cache::{ProjectDrivenCache, ProjectDrivenCacheBackoffice},
        Project, ProjectClaim, ProjectOwnerChange, ProjectSecret, ProjectStatus, ProjectUpdate,
        ProjectUser, ProjectUserInvite, ProjectUserInviteStatus, ProjectUserProject,
        ProjectUserRole, ProvisionalProject,
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
                    p.status, p.billing_provider,
                    p.billing_provider_id,
                    p.billing_subscription_id,
                    p.agent_provisioned,
                    p.agent_name,
                    p.agent_version,
                    p.agent_url,
                    p.expires_at,
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
                    p.agent_provisioned,
                    p.agent_name,
                    p.agent_version,
                    p.agent_url,
                    p.expires_at,
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
                    p.agent_provisioned,
                    p.agent_name,
                    p.agent_version,
                    p.agent_url,
                    p.expires_at,
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

    async fn find_by_claim_token_hash(&self, claim_token_hash: &str) -> Result<Option<Project>> {
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
                    p.agent_provisioned,
                    p.agent_name,
                    p.agent_version,
                    p.agent_url,
                    p.expires_at,
                    p.created_at,
                    p.updated_at
                FROM project p
                WHERE p.claim_token_hash = $1;
            "#,
        )
        .bind(claim_token_hash)
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(project)
    }
    async fn find_by_user_code(&self, user_code: &str) -> Result<Option<Project>> {
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
                    p.agent_provisioned,
                    p.agent_name,
                    p.agent_version,
                    p.agent_url,
                    p.expires_at,
                    p.created_at,
                    p.updated_at
                FROM project p
                WHERE p.user_code = $1;
            "#,
        )
        .bind(user_code)
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

    async fn create_provisional(&self, provisional: &ProvisionalProject) -> Result<()> {
        let project = &provisional.project;
        let status = project.status.to_string();

        // No `project_user` row: nobody is a member until the project is claimed.
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
                    agent_provisioned,
                    agent_name,
                    agent_version,
                    agent_url,
                    expires_at,
                    claim_token_hash,
                    user_code,
                    source_ip_hash,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, NULL, $4, $5, $6, $7, 1, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
            project.id,
            project.namespace,
            project.name,
            status,
            project.billing_provider,
            project.billing_provider_id,
            project.billing_subscription_id,
            project.agent_name,
            project.agent_version,
            project.agent_url,
            project.expires_at,
            provisional.claim_token_hash,
            provisional.user_code,
            provisional.source_ip_hash,
            project.created_at,
            project.updated_at
        )
        .execute(&self.sqlite.db)
        .await?;

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

    async fn change_owner(&self, change: &ProjectOwnerChange) -> Result<()> {
        let mut tx = self.sqlite.db.begin().await?;

        sqlx::query!(
            r#"
                UPDATE project
                SET owner = $1, updated_at = $2
                WHERE id = $3
            "#,
            change.new_owner,
            change.changed_at,
            change.project_id,
        )
        .execute(&mut *tx)
        .await?;

        let owner_role = ProjectUserRole::Owner.to_string();
        sqlx::query!(
            r#"
                UPDATE project_user
                SET role = $1
                WHERE project_id = $2 AND user_id = $3
            "#,
            owner_role,
            change.project_id,
            change.new_owner,
        )
        .execute(&mut *tx)
        .await?;

        let member_role = ProjectUserRole::Member.to_string();
        sqlx::query!(
            r#"
                UPDATE project_user
                SET role = $1
                WHERE project_id = $2 AND user_id = $3
            "#,
            member_role,
            change.project_id,
            change.previous_owner,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
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

    async fn expire(&self, id: &str, expired_at: &DateTime<Utc>) -> Result<()> {
        let expired = ProjectStatus::Expired.to_string();
        let provisional = ProjectStatus::Provisional.to_string();

        let mut tx = self.sqlite.db.begin().await?;

        // Only a provisional project expires, so a duplicate event, or one that lost a race with
        // the claim, changes nothing.
        let result = sqlx::query!(
            r#"
                UPDATE project
                SET status = $1, claim_token_hash = NULL, user_code = NULL, updated_at = $2
                WHERE id = $3 AND status = $4;
            "#,
            expired,
            expired_at,
            id,
            provisional
        )
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() > 0 {
            let status = ResourceStatus::Deleted.to_string();
            sqlx::query!(
                r#"
                    UPDATE resource
                    SET status = $2, updated_at = $3
                    WHERE project_id = $1;
                "#,
                id,
                status,
                expired_at
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn claim(&self, claim: &ProjectClaim) -> Result<()> {
        let active = ProjectStatus::Active.to_string();
        let provisional = ProjectStatus::Provisional.to_string();

        let mut tx = self.sqlite.db.begin().await?;

        // Only a provisional project is claimed, so a duplicate event, or one that lost a race
        // with the expiry, changes nothing. The agent columns and the source hash stay; resources
        // and secrets are not touched.
        let result = sqlx::query!(
            r#"
                UPDATE project
                SET owner = $1,
                    name = $2,
                    status = $3,
                    billing_provider = $4,
                    billing_provider_id = $5,
                    claim_token_hash = NULL,
                    user_code = NULL,
                    expires_at = NULL,
                    updated_at = $6
                WHERE id = $7 AND status = $8;
            "#,
            claim.owner,
            claim.name,
            active,
            claim.billing_provider,
            claim.billing_provider_id,
            claim.claimed_at,
            claim.project_id,
            provisional
        )
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() > 0 {
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
                claim.project_id,
                claim.owner,
                role,
                claim.claimed_at
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn find_expired_provisional(&self, now: &DateTime<Utc>) -> Result<Vec<Project>> {
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
                    p.agent_provisioned,
                    p.agent_name,
                    p.agent_version,
                    p.agent_url,
                    p.expires_at,
                    p.created_at,
                    p.updated_at
                FROM project p
                WHERE p.status = $1 AND DATETIME(p.expires_at) < DATETIME($2);
            "#,
        )
        .bind(ProjectStatus::Provisional.to_string())
        .bind(now)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(projects)
    }

    async fn count_live_provisional(&self) -> Result<u32> {
        let count: i64 = sqlx::query_scalar(
            r#"
                SELECT COUNT(*) FROM project p WHERE p.status = $1;
            "#,
        )
        .bind(ProjectStatus::Provisional.to_string())
        .fetch_one(&self.sqlite.db)
        .await?;

        Ok(count as u32)
    }

    async fn count_registrations_by_source(
        &self,
        source_ip_hash: &str,
        since: &DateTime<Utc>,
    ) -> Result<u32> {
        // Every registration counts, whatever became of its project since.
        let count: i64 = sqlx::query_scalar(
            r#"
                SELECT COUNT(*) FROM project p
                WHERE p.source_ip_hash = $1 AND DATETIME(p.created_at) >= DATETIME($2);
            "#,
        )
        .bind(source_ip_hash)
        .bind(since)
        .fetch_one(&self.sqlite.db)
        .await?;

        Ok(count as u32)
    }

    async fn find_registration_created_at_by_source(
        &self,
        source_ip_hash: &str,
        since: &DateTime<Utc>,
        offset: u32,
    ) -> Result<Option<DateTime<Utc>>> {
        // The same rows `count_registrations_by_source` counts, oldest first.
        let created_at: Option<DateTime<Utc>> = sqlx::query_scalar(
            r#"
                SELECT p.created_at FROM project p
                WHERE p.source_ip_hash = $1 AND DATETIME(p.created_at) >= DATETIME($2)
                ORDER BY DATETIME(p.created_at) ASC, p.created_at ASC
                LIMIT 1 OFFSET $3;
            "#,
        )
        .bind(source_ip_hash)
        .bind(since)
        .bind(offset)
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(created_at)
    }

    async fn exists_user_code(&self, user_code: &str) -> Result<bool> {
        // The claim and the expiry clear the code, so any row still holding it is live.
        let count: i64 = sqlx::query_scalar(
            r#"
                SELECT COUNT(*) FROM project p WHERE p.user_code = $1;
            "#,
        )
        .bind(user_code)
        .fetch_one(&self.sqlite.db)
        .await?;

        Ok(count > 0)
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
    async fn find_secrets(&self, project: &str) -> Result<Vec<ProjectSecret>> {
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
                INNER JOIN project p ON p.id = ps.project_id
                WHERE ps.project_id = $1 OR p.namespace = $1
                ORDER BY ps.created_at DESC;
            "#,
        )
        .bind(project)
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
        let status = ProjectUserInviteStatus::Sent.to_string();

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
                WHERE pui.project_id = $1 AND DATETIME($5) <= DATETIME(pui.expires_in) AND pui.status = $2
                ORDER BY pui.created_at DESC
                LIMIT $3
                OFFSET $4;
            "#,
        )
        .bind(project_id)
        .bind(status)
        .bind(page_size)
        .bind(offset)
        .bind(chrono::Utc::now())
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

    async fn delete_user_invite(&self, invite_id: &str) -> Result<()> {
        sqlx::query!(
            r#"
                DELETE FROM
                    project_user_invite
                WHERE 
                    id=$1;
            "#,
            invite_id,
        )
        .execute(&self.sqlite.db)
        .await?;

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
#[async_trait::async_trait]
impl ProjectDrivenCacheBackoffice for SqliteProjectDrivenCache {
    async fn find_by_user_id(&self, id: &str) -> Result<Vec<Project>> {
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
                    p.agent_provisioned,
                    p.agent_name,
                    p.agent_version,
                    p.agent_url,
                    p.expires_at,
                    p.created_at,
                    p.updated_at
                FROM 
                	  project p
                INNER JOIN project_user pu ON
                	  pu.project_id = p.id
                WHERE
                	  pu.user_id = $1;
            "#,
        )
        .bind(id)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(projects)
    }

    async fn find_by_namespace(&self, id: &str) -> Result<Option<Project>> {
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
                    p.agent_provisioned,
                    p.agent_name,
                    p.agent_version,
                    p.agent_url,
                    p.expires_at,
                    p.created_at,
                    p.updated_at
                FROM 
                	  project p
                WHERE
                	  p.namespace = $1;
            "#,
        )
        .bind(id)
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(project)
    }

    async fn find_by_resource_spec(&self, value: &str) -> Result<Vec<Project>> {
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
	                  p.agent_provisioned,
	                  p.agent_name,
	                  p.agent_version,
	                  p.agent_url,
	                  p.expires_at,
	                  p.created_at,
	                  p.updated_at
                FROM
	                  project p
                WHERE
                    p.id in (SELECT project_id FROM resource r WHERE r.spec LIKE $1);
            "#,
        )
        .bind(format!("%{value}%"))
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(projects)
    }

    async fn find_by_resource_name(&self, resource_name: &str) -> Result<Vec<Project>> {
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
	                  p.agent_provisioned,
	                  p.agent_name,
	                  p.agent_version,
	                  p.agent_url,
	                  p.expires_at,
	                  p.created_at,
	                  p.updated_at
                FROM
	                  project p
                WHERE
                    p.id in (SELECT project_id FROM resource r WHERE r.name = $1);
            "#,
        )
        .bind(resource_name)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(projects)
    }

    async fn find_new_users(&self, after: &str) -> Result<Vec<ProjectUserProject>> {
        let users = sqlx::query_as::<_, ProjectUserProject>(
            r#"
                SELECT
                    pu.user_id,
                    pu.project_id,
                    pu."role",
                    pu.created_at,
                    COUNT(spu.user_id) AS quantity,
                    p.name as project_name, 
                    p.namespace as project_namespace, 
                    p.owner as project_owner, 
                    p.status as project_status, 
                    p.billing_provider as project_billing_provider, 
                    p.billing_provider_id as project_billing_provider_id
                FROM
                    project_user pu
                LEFT JOIN
                    project_user spu ON 
                    spu.user_id = pu.user_id AND DATETIME(spu.created_at) < DATETIME($1) AND spu."role" = 'owner'
                INNER JOIN 
	                  project p ON p.id = pu.project_id AND p.owner == pu.user_id
                WHERE 
                    DATETIME(pu.created_at) >= DATETIME($1)
                GROUP BY
                    pu.user_id, pu.project_id, pu."role", pu.created_at
                HAVING 
                    quantity = 0;
            "#,
        )
        .bind(after)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(users)
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
            agent_provisioned: row.try_get("agent_provisioned")?,
            agent_name: row.try_get("agent_name")?,
            agent_version: row.try_get("agent_version")?,
            agent_url: row.try_get("agent_url")?,
            expires_at: row.try_get("expires_at")?,
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

impl FromRow<'_, SqliteRow> for ProjectUserProject {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        let role: &str = row.try_get("role")?;
        let project_status: &str = row.try_get("project_status")?;

        Ok(Self {
            user_id: row.try_get("user_id")?,
            role: role
                .parse()
                .map_err(|err: Error| sqlx::Error::Decode(err.into()))?,
            created_at: row.try_get("created_at")?,
            project_id: row.try_get("project_id")?,
            project_name: row.try_get("project_name")?,
            project_namespace: row.try_get("project_namespace")?,
            project_owner: row.try_get("project_owner")?,
            project_status: project_status
                .parse()
                .map_err(|err: Error| sqlx::Error::Decode(err.into()))?,
            project_billing_provider: row.try_get("project_billing_provider")?,
            project_billing_provider_id: row.try_get("project_billing_provider_id")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{event::ProvisionalProjectCreated, resource::cache::ResourceDrivenCache},
        driven::cache::{resource::SqliteResourceDrivenCache, tests::mock_resource},
    };

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
        let result = cache.find(project.owner.as_deref().unwrap(), &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().len() == 1);
    }
    #[tokio::test]
    async fn it_should_return_none_find_user_projects_invalid_page() {
        let cache = get_cache().await;
        let project = Project::default();

        cache.create(&project).await.unwrap();
        let result = cache.find(project.owner.as_deref().unwrap(), &2, &12).await;

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
    async fn it_should_change_project_owner() {
        let cache = get_cache().await;
        let project = Project::default();
        cache.create(&project).await.unwrap();

        // The new owner must already be a member of the project.
        let new_owner = ProjectUser {
            user_id: "new user id".into(),
            project_id: project.id.clone(),
            role: ProjectUserRole::Member,
            created_at: Utc::now(),
        };
        cache
            .create_user_acceptance("invite id", &new_owner)
            .await
            .unwrap();

        let change = ProjectOwnerChange {
            project_id: project.id.clone(),
            previous_owner: project.owner.clone().unwrap(),
            new_owner: new_owner.user_id.clone(),
            changed_at: Utc::now(),
        };
        let result = cache.change_owner(&change).await;
        assert!(result.is_ok());

        let updated = cache.find_by_id(&project.id).await.unwrap().unwrap();
        assert_eq!(updated.owner, Some(new_owner.user_id.clone()));

        let new_permission = cache
            .find_user_permission(&new_owner.user_id, &project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(new_permission.role, ProjectUserRole::Owner);

        let previous_permission = cache
            .find_user_permission(project.owner.as_deref().unwrap(), &project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(previous_permission.role, ProjectUserRole::Member);
    }

    #[tokio::test]
    async fn it_should_change_project_owner_idempotently() {
        let cache = get_cache().await;
        let project = Project::default();
        cache.create(&project).await.unwrap();

        let new_owner = ProjectUser {
            user_id: "new user id".into(),
            project_id: project.id.clone(),
            role: ProjectUserRole::Member,
            created_at: Utc::now(),
        };
        cache
            .create_user_acceptance("invite id", &new_owner)
            .await
            .unwrap();

        let change = ProjectOwnerChange {
            project_id: project.id.clone(),
            previous_owner: project.owner.clone().unwrap(),
            new_owner: new_owner.user_id.clone(),
            changed_at: Utc::now(),
        };

        // Replaying the same event must converge on the same state.
        cache.change_owner(&change).await.unwrap();
        cache.change_owner(&change).await.unwrap();

        let updated = cache.find_by_id(&project.id).await.unwrap().unwrap();
        assert_eq!(updated.owner, Some(new_owner.user_id.clone()));

        let new_permission = cache
            .find_user_permission(&new_owner.user_id, &project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(new_permission.role, ProjectUserRole::Owner);

        let previous_permission = cache
            .find_user_permission(project.owner.as_deref().unwrap(), &project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(previous_permission.role, ProjectUserRole::Member);
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
        let cache: Box<dyn ProjectDrivenCache> = Box::new(get_cache().await);
        let project = Project::default();

        cache.create(&project).await.unwrap();
        let result = cache.find_by_namespace(&project.namespace).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
    #[tokio::test]
    async fn it_should_return_none_find_project_by_namespace() {
        let cache: Box<dyn ProjectDrivenCache> = Box::new(get_cache().await);
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
            .find_user_permission(project.owner.as_deref().unwrap(), &project.id)
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
    #[tokio::test]
    async fn it_should_return_none_find_user_permission() {
        let cache = get_cache().await;

        let project = Project::default();

        let result = cache
            .find_user_permission(project.owner.as_deref().unwrap(), &project.id)
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
    async fn it_should_delete_user_invite() {
        let cache = get_cache().await;

        let project = Project::default();
        cache.create(&project).await.unwrap();

        let invite = ProjectUserInvite {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create_user_invite(&invite).await.unwrap();

        let result = cache.delete_user_invite(&invite.id).await;

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

    async fn get_sqlite() -> Arc<SqliteCache> {
        Arc::new(SqliteCache::ephemeral().await.unwrap())
    }

    fn provisional(expires_at: DateTime<Utc>) -> ProvisionalProject {
        ProvisionalProjectCreated {
            namespace: crate::domain::utils::get_random_name(),
            user_code: crate::domain::provisional::generate_user_code(),
            expires_at,
            ..Default::default()
        }
        .try_into()
        .unwrap()
    }

    async fn claim_columns(
        sqlite: &SqliteCache,
        id: &str,
    ) -> (Option<String>, Option<String>, Option<String>, i64) {
        sqlx::query_as(
            "SELECT claim_token_hash, user_code, source_ip_hash, agent_provisioned FROM project WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&sqlite.db)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn it_should_store_a_provisional_project_with_no_owner_and_no_member() {
        let sqlite = get_sqlite().await;
        let cache = SqliteProjectDrivenCache::new(sqlite.clone());
        let created = provisional(Utc::now() + chrono::Duration::days(7));

        cache.create_provisional(&created).await.unwrap();

        let project = cache
            .find_by_id(&created.project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(project.owner, None);
        assert_eq!(project.status, ProjectStatus::Provisional);
        assert!(project.agent_provisioned);
        assert_eq!(project.agent_name.as_deref(), Some("claude-code"));
        assert_eq!(project.agent_version.as_deref(), Some("2.1.0"));
        assert_eq!(project.expires_at, created.project.expires_at);
        assert!(cache
            .find_users(&project.id, &1, &12)
            .await
            .unwrap()
            .is_empty());

        let (claim_token_hash, user_code, source_ip_hash, agent_provisioned) =
            claim_columns(&sqlite, &project.id).await;
        assert_eq!(claim_token_hash, Some(created.claim_token_hash));
        assert_eq!(user_code, Some(created.user_code.clone()));
        assert_eq!(source_ip_hash, Some(created.source_ip_hash));
        assert_eq!(agent_provisioned, 1);
        assert!(cache.exists_user_code(&created.user_code).await.unwrap());

        // An owned project keeps its owner and is not agent-provisioned.
        let owned = Project::default();
        cache.create(&owned).await.unwrap();
        let owned = cache.find_by_id(&owned.id).await.unwrap().unwrap();
        assert_eq!(owned.owner.as_deref(), Some("user id"));
        assert!(!owned.agent_provisioned);
        assert_eq!(owned.expires_at, None);
    }

    #[tokio::test]
    async fn it_should_expire_a_provisional_project_and_delete_its_resources() {
        let sqlite = get_sqlite().await;
        let cache = SqliteProjectDrivenCache::new(sqlite.clone());
        let resources = SqliteResourceDrivenCache::new(sqlite.clone());
        let created = provisional(Utc::now() - chrono::Duration::hours(1));
        cache.create_provisional(&created).await.unwrap();
        let resource = mock_resource(sqlite.clone(), &created.project.id).await;

        let expired_at = Utc::now();
        cache
            .expire(&created.project.id, &expired_at)
            .await
            .unwrap();

        let project = cache
            .find_by_id(&created.project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(project.status, ProjectStatus::Expired);
        assert_eq!(project.updated_at, expired_at);
        let (claim_token_hash, user_code, source_ip_hash, _) =
            claim_columns(&sqlite, &project.id).await;
        assert_eq!(claim_token_hash, None);
        assert_eq!(user_code, None);
        assert!(source_ip_hash.is_some(), "the IP cap still counts it");
        assert!(resources.find_by_id(&resource.id).await.unwrap().is_none());
        assert_eq!(resources.count_live(&project.id).await.unwrap(), 0);

        // A duplicate changes nothing.
        cache
            .expire(
                &created.project.id,
                &(expired_at + chrono::Duration::minutes(10)),
            )
            .await
            .unwrap();
        let again = cache
            .find_by_id(&created.project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(again.updated_at, expired_at);
    }

    #[tokio::test]
    async fn it_should_not_expire_an_owned_project() {
        let sqlite = get_sqlite().await;
        let cache = SqliteProjectDrivenCache::new(sqlite.clone());
        let project = Project::default();
        cache.create(&project).await.unwrap();
        let resource = mock_resource(sqlite.clone(), &project.id).await;

        cache.expire(&project.id, &Utc::now()).await.unwrap();

        let project = cache.find_by_id(&project.id).await.unwrap().unwrap();
        assert_eq!(project.status, ProjectStatus::Active);
        let resources = SqliteResourceDrivenCache::new(sqlite);
        assert!(resources.find_by_id(&resource.id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_should_find_only_provisional_projects_past_their_expiry() {
        let cache = get_cache().await;
        let now = Utc::now();

        let past = provisional(now - chrono::Duration::minutes(1));
        let future = provisional(now + chrono::Duration::minutes(1));
        let already = provisional(now - chrono::Duration::days(1));
        for p in [&past, &future, &already] {
            cache.create_provisional(p).await.unwrap();
        }
        cache.expire(&already.project.id, &now).await.unwrap();
        cache.create(&Project::default()).await.unwrap();

        let found = cache.find_expired_provisional(&now).await.unwrap();
        let ids: Vec<&str> = found.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec![past.project.id.as_str()]);
    }

    #[tokio::test]
    async fn it_should_count_live_provisional_projects_and_registrations_by_source() {
        let cache = get_cache().await;
        let now = Utc::now();

        let live = provisional(now + chrono::Duration::days(7));
        let expired = provisional(now - chrono::Duration::days(1));
        let mut old = provisional(now + chrono::Duration::days(7));
        old.project.created_at = now - chrono::Duration::hours(25);
        let mut elsewhere = provisional(now + chrono::Duration::days(7));
        elsewhere.source_ip_hash = "another hash".into();
        for p in [&live, &expired, &old, &elsewhere] {
            cache.create_provisional(p).await.unwrap();
        }
        cache.expire(&expired.project.id, &now).await.unwrap();
        cache.create(&Project::default()).await.unwrap();

        assert_eq!(cache.count_live_provisional().await.unwrap(), 3);

        let since = now - chrono::Duration::hours(24);
        // The expired registration still counts; the one from 25 hours ago does not.
        let count = cache
            .count_registrations_by_source(&live.source_ip_hash, &since)
            .await
            .unwrap();
        assert_eq!(count, 2);
        let count = cache
            .count_registrations_by_source("another hash", &since)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn it_should_find_registration_times_by_source_oldest_first() {
        let cache = get_cache().await;
        let now = Utc::now();
        let since = now - chrono::Duration::hours(24);

        let mut created = Vec::new();
        for hours in [25, 1, 23, 12] {
            let mut p = provisional(now + chrono::Duration::days(7));
            p.project.created_at = now - chrono::Duration::hours(hours);
            cache.create_provisional(&p).await.unwrap();
            created.push(p);
        }
        // An expired registration still counts, as it does for the cap.
        cache.expire(&created[2].project.id, &now).await.unwrap();
        let mut elsewhere = provisional(now + chrono::Duration::days(7));
        elsewhere.source_ip_hash = "another hash".into();
        elsewhere.project.created_at =
            now - chrono::Duration::hours(23) - chrono::Duration::minutes(30);
        cache.create_provisional(&elsewhere).await.unwrap();

        let hash = &created[0].source_ip_hash;
        let found = |offset| cache.find_registration_created_at_by_source(hash, &since, offset);
        // The one from 25 hours ago is outside the window; the rest come oldest first.
        assert_eq!(found(0).await.unwrap(), Some(created[2].project.created_at));
        assert_eq!(found(1).await.unwrap(), Some(created[3].project.created_at));
        assert_eq!(found(2).await.unwrap(), Some(created[1].project.created_at));
        assert_eq!(found(3).await.unwrap(), None);

        let none = cache
            .find_registration_created_at_by_source("no such hash", &since, 0)
            .await
            .unwrap();
        assert_eq!(none, None);
    }

    /// Every column of every row of `table` that belongs to the project, as raw bytes.
    async fn raw_rows(sqlite: &SqliteCache, table: &str, project_id: &str) -> Vec<Vec<Vec<u8>>> {
        sqlx::query(&format!(
            "SELECT * FROM {table} WHERE project_id = $1 ORDER BY id"
        ))
        .bind(project_id)
        .fetch_all(&sqlite.db)
        .await
        .unwrap()
        .iter()
        .map(|row| {
            (0..row.len())
                .map(|i| {
                    row.try_get_unchecked::<Option<Vec<u8>>, _>(i)
                        .unwrap()
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect()
    }

    fn claim_of(provisional: &ProvisionalProject) -> ProjectClaim {
        ProjectClaim {
            project_id: provisional.project.id.clone(),
            owner: "claimer id".into(),
            name: "My Project".into(),
            billing_provider: "stripe".into(),
            billing_provider_id: "cus_123".into(),
            claimed_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn it_should_find_a_provisional_project_by_claim_token_hash_and_user_code() {
        let cache = get_cache().await;
        let created = provisional(Utc::now() + chrono::Duration::days(7));
        cache.create_provisional(&created).await.unwrap();

        let by_hash = cache
            .find_by_claim_token_hash(&created.claim_token_hash)
            .await
            .unwrap();
        assert_eq!(by_hash.map(|p| p.id), Some(created.project.id.clone()));
        let by_code = cache.find_by_user_code(&created.user_code).await.unwrap();
        assert_eq!(by_code.map(|p| p.id), Some(created.project.id.clone()));
        assert!(cache
            .find_by_claim_token_hash("unknown")
            .await
            .unwrap()
            .is_none());
        assert!(cache.find_by_user_code("ZZZZ2222").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_should_claim_a_provisional_project_and_leave_its_resources_and_secrets() {
        let sqlite = get_sqlite().await;
        let cache = SqliteProjectDrivenCache::new(sqlite.clone());
        let created = provisional(Utc::now() + chrono::Duration::days(7));
        let project_id = created.project.id.clone();
        cache.create_provisional(&created).await.unwrap();
        mock_resource(sqlite.clone(), &project_id).await;
        mock_resource(sqlite.clone(), &project_id).await;
        cache
            .create_secret(&ProjectSecret {
                project_id: project_id.clone(),
                ..Default::default()
            })
            .await
            .unwrap();
        let resources_before = raw_rows(&sqlite, "resource", &project_id).await;
        let secrets_before = raw_rows(&sqlite, "project_secret", &project_id).await;
        assert_eq!(resources_before.len(), 2);
        assert_eq!(secrets_before.len(), 1);

        let claim = claim_of(&created);
        cache.claim(&claim).await.unwrap();

        let project = cache.find_by_id(&project_id).await.unwrap().unwrap();
        assert_eq!(project.owner.as_deref(), Some("claimer id"));
        assert_eq!(project.name, "My Project");
        assert_eq!(project.status, ProjectStatus::Active);
        assert_eq!(project.billing_provider, "stripe");
        assert_eq!(project.billing_provider_id, "cus_123");
        assert_eq!(project.updated_at, claim.claimed_at);
        assert_eq!(project.expires_at, None);
        assert!(project.agent_provisioned);
        assert_eq!(project.agent_name.as_deref(), Some("claude-code"));
        assert_eq!(project.agent_version.as_deref(), Some("2.1.0"));

        let (claim_token_hash, user_code, source_ip_hash, agent_provisioned) =
            claim_columns(&sqlite, &project_id).await;
        assert_eq!(claim_token_hash, None);
        assert_eq!(user_code, None);
        assert_eq!(source_ip_hash, Some(created.source_ip_hash.clone()));
        assert_eq!(agent_provisioned, 1);
        assert!(cache
            .find_by_claim_token_hash(&created.claim_token_hash)
            .await
            .unwrap()
            .is_none());
        assert!(!cache.exists_user_code(&created.user_code).await.unwrap());

        let users = cache.find_users(&project_id, &1, &12).await.unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].user_id, "claimer id");
        assert_eq!(users[0].role, ProjectUserRole::Owner);
        let listed = cache.find("claimer id", &1, &12).await.unwrap();
        assert_eq!(listed.len(), 1);

        assert_eq!(
            raw_rows(&sqlite, "resource", &project_id).await,
            resources_before
        );
        assert_eq!(
            raw_rows(&sqlite, "project_secret", &project_id).await,
            secrets_before
        );

        // A duplicate changes nothing, and adds no second member.
        let duplicate = ProjectClaim {
            owner: "someone else".into(),
            claimed_at: Utc::now() + chrono::Duration::minutes(10),
            ..claim.clone()
        };
        cache.claim(&duplicate).await.unwrap();
        let again = cache.find_by_id(&project_id).await.unwrap().unwrap();
        assert_eq!(again.owner.as_deref(), Some("claimer id"));
        assert_eq!(again.updated_at, claim.claimed_at);
        assert_eq!(
            cache.find_users(&project_id, &1, &12).await.unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn it_should_not_claim_an_expired_project() {
        let cache = get_cache().await;
        let created = provisional(Utc::now() - chrono::Duration::hours(1));
        cache.create_provisional(&created).await.unwrap();
        cache
            .expire(&created.project.id, &Utc::now())
            .await
            .unwrap();

        cache.claim(&claim_of(&created)).await.unwrap();

        let project = cache
            .find_by_id(&created.project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(project.status, ProjectStatus::Expired);
        assert_eq!(project.owner, None);
        assert!(cache
            .find_users(&created.project.id, &1, &12)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn it_should_leave_a_claimed_project_out_of_the_expiry_sweep() {
        let sqlite = get_sqlite().await;
        let cache = SqliteProjectDrivenCache::new(sqlite.clone());
        let expires_at = Utc::now() + chrono::Duration::minutes(1);
        let created = provisional(expires_at);
        cache.create_provisional(&created).await.unwrap();
        let resource = mock_resource(sqlite.clone(), &created.project.id).await;
        cache.claim(&claim_of(&created)).await.unwrap();

        let after_old_expiry = expires_at + chrono::Duration::hours(1);
        assert!(cache
            .find_expired_provisional(&after_old_expiry)
            .await
            .unwrap()
            .is_empty());

        // Even a late `ProjectExpired` from a sweep that raced the claim changes nothing.
        cache
            .expire(&created.project.id, &after_old_expiry)
            .await
            .unwrap();
        let project = cache
            .find_by_id(&created.project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(project.status, ProjectStatus::Active);
        let resources = SqliteResourceDrivenCache::new(sqlite);
        assert!(resources.find_by_id(&resource.id).await.unwrap().is_some());
    }
}
