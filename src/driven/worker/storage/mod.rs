use anyhow::Result;

pub mod keyvalue;
pub mod logs;

pub struct PostgresStorage {
    pool: sqlx::postgres::PgPool,
}

impl PostgresStorage {
    pub async fn new(url: &str) -> Result<Self> {
        let pool = sqlx::postgres::PgPool::connect(url).await?;
        Ok(Self { pool })
    }
}
