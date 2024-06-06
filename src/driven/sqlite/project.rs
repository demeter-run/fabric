use anyhow::Result;
use std::sync::Arc;

use crate::domain::management::project::{Project, ProjectState};

use super::SqliteState;

pub struct SqliteProjectState {
    sqlite: Arc<SqliteState>,
}
impl SqliteProjectState {
    pub fn new(sqlite: Arc<SqliteState>) -> Self {
        Self { sqlite }
    }
}
#[async_trait::async_trait]
impl ProjectState for SqliteProjectState {
    async fn create(&self, project: &Project) -> Result<()> {
        sqlx::query!(
            r#"
                INSERT INTO projects (slug, name, description)
                VALUES ($1, $2, $3)
            "#,
            project.slug,
            project.name,
            project.description
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }
    async fn find_by_slug(&self, slug: &str) -> Result<Option<Project>> {
        todo!()
    }
}
