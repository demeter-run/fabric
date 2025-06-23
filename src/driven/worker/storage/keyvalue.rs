use std::sync::Arc;

use crate::domain::{
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
    async fn find(&self, worker_id: &str, page: &u32, page_size: &u32) -> Result<Vec<KeyValue>> {
        todo!()
    }

    async fn update(&self, key_value: &KeyValue) -> Result<()> {
        todo!()
    }

    async fn delete(&self, key: &str) -> Result<()> {
        todo!()
    }
}
