use std::str::FromStr;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;

use super::{error::Error, Result};

pub mod command;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait MetadataDriven: Send + Sync {
    async fn find(&self) -> Result<Vec<CustomResourceDefinition>>;
    async fn find_by_kind(&self, kind: &str) -> Result<Option<CustomResourceDefinition>>;
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

#[cfg(test)]
pub mod tests {
    use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;

    const CARDANO_NODE_PORT_CRD: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/crd/cardanonodeport.json"
    ));

    pub fn mock_crd() -> CustomResourceDefinition {
        serde_json::from_str(CARDANO_NODE_PORT_CRD).unwrap()
    }
}
