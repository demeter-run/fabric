use std::sync::Arc;

use sqlx::{postgres::PgRow, FromRow, Postgres, Row};

use crate::domain::{
    error::Error,
    worker::storage::{KeyValue, WorkerKeyValueDrivenStorage},
    Result,
};

use super::PostgresStorage;

pub struct PostgresWorkerKeyValueDrivenStorage {
    storage: Arc<PostgresStorage>,
}
impl PostgresWorkerKeyValueDrivenStorage {
    pub fn new(storage: Arc<PostgresStorage>) -> Self {
        Self { storage }
    }
}

#[async_trait::async_trait]
impl WorkerKeyValueDrivenStorage for PostgresWorkerKeyValueDrivenStorage {
    async fn find(
        &self,
        worker_id: &str,
        key: Option<String>,
        page: &u32,
        page_size: &u32,
    ) -> Result<(i64, Vec<KeyValue>)> {
        let offset = (page_size * (page - 1)) as i64;
        let page_size = *page_size as i64;

        let mut query = String::from(
            r#"
            SELECT 
                kv.worker, 
                kv."key", 
                kv.value,
                COUNT(*) OVER () AS total_count
            FROM
                kv
            WHERE kv.worker = $1
        "#,
        );

        if key.is_some() {
            query.push_str(" AND kv.\"key\" ILIKE $4");
        }

        query.push_str(" LIMIT $2 OFFSET $3;");

        let mut q = sqlx::query(&query)
            .bind(worker_id)
            .bind(page_size)
            .bind(offset);

        if let Some(k) = &key {
            q = q.bind(format!("%{k}%"));
        }

        let rows = q.fetch_all(&self.storage.pool).await?;

        let count = rows
            .first()
            .map(|r| r.get::<i64, _>("total_count"))
            .unwrap_or_default();

        let values = rows
            .into_iter()
            .map(|r| KeyValue::from_row(&r))
            .collect::<sqlx::Result<Vec<KeyValue>>>()?;

        Ok((count, values))
    }

    async fn update(&self, key_value: &KeyValue) -> Result<KeyValue> {
        let updated = sqlx::query_as::<_, KeyValue>(
            r#"
                UPDATE kv
                SET value = $3
                WHERE worker = $1 AND "key" = $2
                RETURNING worker, "key", value;
            "#,
        )
        .bind(&key_value.worker_id)
        .bind(&key_value.key)
        .bind(&key_value.value)
        .fetch_optional(&self.storage.pool)
        .await?;

        match updated {
            Some(value) => Ok(value),
            None => Err(Error::CommandMalformed("key not found".into())),
        }
    }

    async fn delete(&self, worker_id: &str, key: &str) -> Result<()> {
        let result = sqlx::query::<Postgres>(
            r#"
                DELETE FROM
                   kv 
                WHERE 
                    worker = $1 AND key = $2;
            "#,
        )
        .bind(worker_id)
        .bind(key)
        .execute(&self.storage.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(Error::CommandMalformed("key not found".into()));
        }

        Ok(())
    }
}

impl FromRow<'_, PgRow> for KeyValue {
    fn from_row(row: &PgRow) -> sqlx::Result<Self> {
        Ok(Self {
            worker_id: row.try_get("worker")?,
            key: row.try_get("key")?,
            value: row.try_get("value")?,
        })
    }
}
