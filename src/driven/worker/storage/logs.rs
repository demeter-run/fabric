use std::sync::Arc;

use sqlx::{postgres::PgRow, FromRow, Postgres, Row};

use crate::domain::{
    worker::logs::{Log, WorkerLogsDriven},
    Result,
};

use super::PostgresStorage;

pub struct PostgresWorkerLogsDrivenStorage {
    storage: Arc<PostgresStorage>,
}
impl PostgresWorkerLogsDrivenStorage {
    pub fn new(storage: Arc<PostgresStorage>) -> Self {
        Self { storage }
    }
}

#[async_trait::async_trait]
impl WorkerLogsDriven for PostgresWorkerLogsDrivenStorage {
    async fn prev(&self, worker_id: &str, cursor: i64, limit: i64) -> Result<Vec<Log>> {
        let logs = sqlx::query_as::<Postgres, Log>(
            r#"
                SELECT
                	  logs."timestamp",
                	  logs.worker,
                	  logs."level",
                	  logs.message,
                	  logs.context
                FROM
                	  logs
                WHERE
                    logs.worker = $1 and "timestamp" <= to_timestamp($2)
                ORDER BY "timestamp" DESC
                LIMIT $3;
            "#,
        )
        .bind(worker_id)
        .bind(cursor)
        .bind(limit)
        .fetch_all(&self.storage.pool)
        .await?;

        Ok(logs)
    }
    async fn next(&self, worker_id: &str, cursor: i64, limit: i64) -> Result<Vec<Log>> {
        let logs = sqlx::query_as::<Postgres, Log>(
            r#"
                SELECT
                	  logs."timestamp",
                	  logs.worker,
                	  logs."level",
                	  logs.message,
                	  logs.context
                FROM
                	  logs
                WHERE
                    logs.worker = $1 and "timestamp" >= to_timestamp($2)
                LIMIT $3;
            "#,
        )
        .bind(worker_id)
        .bind(cursor)
        .bind(limit)
        .fetch_all(&self.storage.pool)
        .await?;

        Ok(logs)
    }
}

impl FromRow<'_, PgRow> for Log {
    fn from_row(row: &PgRow) -> sqlx::Result<Self> {
        Ok(Self {
            worker_id: row.try_get("worker")?,
            timestamp: row.try_get("timestamp")?,
            level: row.try_get("level")?,
            message: row.try_get("message")?,
            context: row.try_get("context")?,
        })
    }
}
