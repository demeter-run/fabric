use std::sync::Arc;

use sqlx::{postgres::PgRow, FromRow, Postgres, Row};

use crate::domain::{
    error::Error,
    worker::{KeyValue, WorkerKeyValueDrivenStorage},
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
    ) -> Result<Vec<KeyValue>> {
        let offset = (page_size * (page - 1)) as i64;
        let page_size = *page_size as i64;

        let mut query = String::from(
            r#"
            SELECT 
                kv.worker, 
                kv."key", 
                kv.value,
                kv.type,
                kv.secure
            FROM
                kv
            WHERE kv.worker = $1
        "#,
        );

        if key.is_some() {
            query.push_str(" AND kv.\"key\" = $4");
        }

        query.push_str(" LIMIT $2 OFFSET $3;");

        let mut q = sqlx::query_as::<_, KeyValue>(&query)
            .bind(worker_id)
            .bind(page_size)
            .bind(offset);

        if let Some(k) = &key {
            q = q.bind(k);
        }

        let values = q.fetch_all(&self.storage.pool).await?;

        Ok(values)
    }

    async fn update(&self, key_value: &KeyValue) -> Result<()> {
        let type_str = key_value.r#type.to_string();

        let result = sqlx::query::<Postgres>(
            r#"
                UPDATE kv
                SET value = $3, type = $4, secure = $5
                WHERE worker = $1 AND key = $2;
            "#,
        )
        .bind(&key_value.worker_id)
        .bind(&key_value.key)
        .bind(&key_value.value)
        .bind(type_str)
        .bind(key_value.secure)
        .execute(&self.storage.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(Error::CommandMalformed("key not found".into()));
        }

        Ok(())
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
        let type_str: &str = row.try_get("type")?;

        Ok(Self {
            worker_id: row.try_get("worker")?,
            key: row.try_get("key")?,
            value: row.try_get("value")?,
            r#type: type_str
                .parse()
                .map_err(|err| sqlx::Error::Decode(Box::new(err)))?,
            secure: row.try_get("secure")?,
        })
    }
}
