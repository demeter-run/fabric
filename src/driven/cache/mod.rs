use anyhow::Result;
use std::path::Path;

pub mod project;
pub mod resource;
pub mod usage;

pub struct SqliteCache {
    db: sqlx::sqlite::SqlitePool,
}

impl SqliteCache {
    pub async fn new(path: &Path) -> Result<Self> {
        let url = format!("sqlite:{}?mode=rwc", path.display());
        let db = sqlx::sqlite::SqlitePoolOptions::new().connect(&url).await?;

        Ok(Self { db })
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("src/driven/cache/migrations")
            .run(&self.db)
            .await?;

        Ok(())
    }

    #[cfg(test)]
    pub async fn ephemeral() -> Result<Self> {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await?;

        let out = Self { db };
        out.migrate().await?;

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use resource::SqliteResourceDrivenCache;

    use crate::{
        domain::{
            project::{cache::ProjectDrivenCache, Project},
            resource::{cache::ResourceDrivenCache, Resource},
        },
        driven::cache::project::SqliteProjectDrivenCache,
    };

    use super::*;

    pub async fn mock_project(sqlite_cache: Arc<SqliteCache>) -> Project {
        let cache: Box<dyn ProjectDrivenCache> =
            Box::new(SqliteProjectDrivenCache::new(sqlite_cache));

        let project = Project::default();
        cache.create(&project).await.unwrap();

        project
    }
    #[tokio::test]
    async fn it_should_keep_existing_owners_through_the_provisional_migration() {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();

        // Every migration before the one that makes the owner nullable.
        let mut migrator = sqlx::migrate!("src/driven/cache/migrations");
        let all = migrator.migrations.clone();
        migrator.migrations = all
            .iter()
            .filter(|m| m.version < 202610161200)
            .cloned()
            .collect::<Vec<_>>()
            .into();
        migrator.run(&db).await.unwrap();

        let sqlite = Arc::new(SqliteCache { db });
        let project = mock_project(sqlite.clone()).await;
        let resource = mock_resource(sqlite.clone(), &project.id).await;

        sqlite.migrate().await.unwrap();

        let cache = SqliteProjectDrivenCache::new(sqlite.clone());
        let migrated = cache.find_by_id(&project.id).await.unwrap().unwrap();
        assert_eq!(migrated.owner, project.owner);
        assert!(!migrated.agent_provisioned);
        assert!(cache
            .find_user_permission(project.owner.as_deref().unwrap(), &project.id)
            .await
            .unwrap()
            .is_some());

        let resources = SqliteResourceDrivenCache::new(sqlite.clone());
        assert!(resources.find_by_id(&resource.id).await.unwrap().is_some());

        let violations: Vec<(String,)> =
            sqlx::query_as("SELECT \"table\" FROM pragma_foreign_key_check")
                .fetch_all(&sqlite.db)
                .await
                .unwrap();
        assert!(violations.is_empty());
    }

    pub async fn mock_resource(sqlite_cache: Arc<SqliteCache>, project_id: &str) -> Resource {
        let cache: Box<dyn ResourceDrivenCache> =
            Box::new(SqliteResourceDrivenCache::new(sqlite_cache));

        let resource = Resource {
            project_id: project_id.to_string(),
            ..Default::default()
        };

        cache.create(&resource).await.unwrap();

        resource
    }
}
