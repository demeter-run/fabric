use std::{fs, path::Path};

use anyhow::Result as AnyhowResult;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use tracing::error;

use crate::domain::{metadata::MetadataDriven, Result};

pub struct MetadataCrd {
    crds: Vec<CustomResourceDefinition>,
}

impl MetadataCrd {
    pub fn new(path: &Path) -> AnyhowResult<Self> {
        let dir = fs::read_dir(path)?;

        let mut crds: Vec<CustomResourceDefinition> = Vec::new();

        for path in dir {
            match path {
                Ok(entry) => {
                    let file = fs::read(entry.path())?;
                    let crd: CustomResourceDefinition = serde_json::from_slice(&file)?;
                    crds.push(crd);
                }
                Err(error) => {
                    error!(?error)
                }
            };
        }

        Ok(Self { crds })
    }
}

#[async_trait::async_trait]
impl MetadataDriven for MetadataCrd {
    async fn find(&self) -> Result<Vec<CustomResourceDefinition>> {
        Ok(self.crds.clone())
    }
    async fn find_by_kind(&self, kind: &str) -> Result<Option<CustomResourceDefinition>> {
        Ok(self
            .crds
            .clone()
            .into_iter()
            .find(|crd| crd.spec.names.kind == kind))
    }
}
