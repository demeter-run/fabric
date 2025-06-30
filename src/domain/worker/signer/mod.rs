use crate::domain::Result;

pub mod command;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait WorkerSignerDrivenStorage: Send + Sync {
    async fn list(&self, worker_id: &str) -> Result<Vec<Signer>>;
    async fn get_public_key(&self, worker_id: &str, key_name: String) -> Result<Option<Vec<u8>>>;
    async fn sign_payload(
        &self,
        worker_id: &str,
        key_name: String,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>>;
}

#[derive(Debug, Clone)]
pub struct Signer {
    pub worker_id: String,
    pub key_name: String,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    impl Default for Signer {
        fn default() -> Self {
            Self {
                worker_id: Uuid::new_v4().to_string(),
                key_name: "key".into(),
            }
        }
    }
}
