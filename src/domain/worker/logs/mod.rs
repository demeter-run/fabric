use chrono::{DateTime, Utc};

use crate::domain::Result;

pub mod command;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait WorkerLogsDrivenStorage: Send + Sync {
    async fn prev(&self, worker_id: &str, cursor: i64, limit: i64) -> Result<Vec<Log>>;
    async fn next(&self, worker_id: &str, cursor: i64, limit: i64) -> Result<Vec<Log>>;
}

#[derive(Debug, Clone)]
pub struct Log {
    pub worker_id: String,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
    pub context: String,
}

#[derive(Debug, Clone)]
pub enum FetchDirection {
    Prev,
    Next,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    impl Default for Log {
        fn default() -> Self {
            Self {
                worker_id: Uuid::new_v4().to_string(),
                timestamp: Utc::now(),
                level: "INFO".to_string(),
                message: "This is a mock log message.".to_string(),
                context: "fn".to_string(),
            }
        }
    }
}
