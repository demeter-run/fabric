use anyhow::Result;

pub mod account;

pub struct DemeterLegacy {
    http_client: reqwest::Client,
    db_pool: sqlx::postgres::PgPool,
}
impl DemeterLegacy {
    pub async fn new(conn_str: &str) -> Result<Self> {
        let db_pool = sqlx::PgPool::connect(conn_str).await?;
        let http_client = reqwest::Client::new();

        Ok(Self {
            http_client,
            db_pool,
        })
    }
}
