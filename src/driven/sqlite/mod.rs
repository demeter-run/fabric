use anyhow::Result;
use std::path::Path;

pub mod project;

pub struct SqliteState {
    db: sqlx::sqlite::SqlitePool,
}

impl SqliteState {
    pub async fn new(path: &Path) -> Result<Self> {
        let url = format!("sqlite:{}?mode=rwc", path.display());
        let db = sqlx::sqlite::SqlitePoolOptions::new().connect(&url).await?;

        Ok(Self { db })
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("src/driven/sqlite/migrations")
            .run(&self.db)
            .await?;

        Ok(())
    }
}
