use crate::domain::Result;

pub mod command;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait WorkerKeyValueDrivenStorage: Send + Sync {
    async fn find(
        &self,
        worker_id: &str,
        key: Option<String>,
        page: &u32,
        page_size: &u32,
    ) -> Result<(i64, Vec<KeyValue>)>;
    async fn update(&self, worker_id: &str, key_value: &KeyValue) -> Result<KeyValue>;
    async fn delete(&self, worker_id: &str, key: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct KeyValue {
    pub worker_id: String,
    pub key: String,
    pub value: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    impl Default for KeyValue {
        fn default() -> Self {
            Self {
                worker_id: Uuid::new_v4().to_string(),
                key: "key".into(),
                value: "test".as_bytes().to_vec(),
            }
        }
    }
}
