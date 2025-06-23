use std::{fmt::Display, str::FromStr};

use super::error::Error;
use crate::domain::Result;

pub mod command;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait WorkerKeyValueDrivenStorage: Send + Sync {
    async fn find(&self, worker_id: &str, page: &u32, page_size: &u32) -> Result<Vec<KeyValue>>;
    async fn update(&self, key_value: &KeyValue) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct KeyValue {
    pub key: String,
    pub value: Vec<u8>,
    pub r#type: KeyValueType,
    pub secure: bool,
}

#[derive(Debug, Clone)]
pub enum KeyValueType {
    String,
    Bytes,
    Int,
    Bool,
}
impl FromStr for KeyValueType {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "string" => Ok(Self::String),
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
            Self::String => write!(f, "string"),
            Self::Bytes => write!(f, "bytes"),
            Self::Int => write!(f, "int"),
            Self::Bool => write!(f, "bool"),
        }
    }
}
