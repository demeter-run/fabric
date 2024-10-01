use std::str::FromStr;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use serde::{Deserialize, Serialize};

use super::{error::Error, Result};

pub mod command;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait MetadataDriven: Send + Sync {
    async fn find(&self) -> Result<Vec<ResourceMetadata>>;
    async fn find_by_kind(&self, kind: &str) -> Result<Option<ResourceMetadata>>;
    fn render_hbs(&self, name: &str, spec: &str) -> Result<String>;
}

pub enum KnownField {
    AuthToken,
    Username,
    Password,
}
impl FromStr for KnownField {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "authToken" => Ok(Self::AuthToken),
            "username" => Ok(Self::Username),
            "password" => Ok(Self::Password),
            _ => Err(Error::Unexpected(format!("status field {s} not supported"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetadata {
    pub options: serde_json::Value,
    pub crd: CustomResourceDefinition,
}

#[cfg(test)]
pub mod tests {
    use super::*;

    const CARDANO_NODE_PORT_CRD: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/crd/cardanonodeport.json"
    ));

    impl Default for ResourceMetadata {
        fn default() -> Self {
            Self {
                crd: serde_json::from_str(CARDANO_NODE_PORT_CRD).unwrap(),
                options: serde_json::Value::default(),
            }
        }
    }
}
