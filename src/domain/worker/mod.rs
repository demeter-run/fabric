use std::{fmt::Display, str::FromStr};

use super::error::Error;
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
    ) -> Result<Vec<KeyValue>>;
    async fn update(&self, key_value: &KeyValue) -> Result<()>;
    async fn delete(&self, worker_id: &str, key: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct KeyValue {
    pub worker_id: String,
    pub key: String,
    pub value: Vec<u8>,
    pub r#type: KeyValueType,
    pub secure: bool,
}

#[derive(Debug, Clone)]
pub enum KeyValueType {
    Text,
    Bytes,
    Int,
    Bool,
}
impl FromStr for KeyValueType {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "text" => Ok(Self::Text),
            "bytes" => Ok(Self::Bytes),
            "int" => Ok(Self::Int),
            "bool" => Ok(Self::Bool),
            _ => Err(Error::Unexpected("key value type not supported".into())),
        }
    }
}
impl Display for KeyValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Bytes => write!(f, "bytes"),
            Self::Int => write!(f, "int"),
            Self::Bool => write!(f, "bool"),
        }
    }
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
                r#type: KeyValueType::Text,
                secure: false,
            }
        }
    }
}
