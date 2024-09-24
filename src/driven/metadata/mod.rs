use std::{fs, path::Path};

use anyhow::{Error, Result as AnyhowResult};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;

use crate::domain::{metadata::MetadataDriven, Result};

pub struct Metadata<'a> {
    crds: Vec<CustomResourceDefinition>,
    hbs: handlebars::Handlebars<'a>,
}

impl Metadata<'_> {
    pub fn new(path: &Path) -> AnyhowResult<Self> {
        let dir = fs::read_dir(path)?;

        let mut crds: Vec<CustomResourceDefinition> = Vec::new();
        let mut hbs = handlebars::Handlebars::new();

        for path in dir {
            let entry = path?;
            if entry.path().is_file() {
                let file = fs::read(entry.path())?;

                match entry.path().extension().and_then(|e| e.to_str()) {
                    Some("json") => {
                        let crd: CustomResourceDefinition = serde_json::from_slice(&file)?;
                        crds.push(crd);
                    }
                    Some("hbs") => {
                        let name = entry.file_name().to_str().unwrap().to_string();
                        let template = String::from_utf8(file.clone())?;

                        hbs.register_template_string(&name, template)?;
                    }
                    _ => continue,
                };
            }
        }

        Ok(Self { crds, hbs })
    }
}

#[async_trait::async_trait]
impl MetadataDriven for Metadata<'_> {
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

    async fn render_hbs(&self, name: &str, spec: &str) -> Result<String> {
        let data: serde_json::Value = serde_json::from_str(spec)?;

        let x = self.hbs.render(name, &data)?.clone();
        Ok(x)
    }
}
