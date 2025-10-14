use std::{collections::HashMap, fs, path::Path};

use anyhow::Result as AnyhowResult;
use include_dir::Dir;
use lazy_static::lazy_static;

use crate::domain::{
    error::Error,
    metadata::{MetadataDriven, ResourceMetadata},
    resource::Resource,
    Result,
};

lazy_static! {
    static ref LEGACY_NETWORKS: HashMap<&'static str, String> = {
        let mut m = HashMap::new();
        m.insert("mainnet", "cardano-mainnet".into());
        m.insert("preprod", "cardano-preprod".into());
        m.insert("preview", "cardano-preview".into());
        m
    };
}

pub fn network_with_chain_prefix(network: &str) -> String {
    let default = network.to_string();
    LEGACY_NETWORKS.get(network).unwrap_or(&default).to_string()
}

#[derive(Debug)]
pub struct FileMetadata<'a> {
    metadata: Vec<ResourceMetadata>,
    hbs: handlebars::Handlebars<'a>,
}

impl FileMetadata<'_> {
    pub fn new(path: &Path) -> AnyhowResult<Self> {
        let dir = fs::read_dir(path)?;

        let mut metadata: Vec<ResourceMetadata> = Vec::new();
        let mut hbs = handlebars::Handlebars::new();

        for path in dir {
            let entry = path?;
            if entry.path().is_file() {
                let file = fs::read(entry.path())?;

                match entry.path().extension().and_then(|e| e.to_str()) {
                    Some("json") => {
                        metadata.push(serde_json::from_slice(&file)?);
                    }
                    Some("hbs") => {
                        let name = entry
                            .path()
                            .file_stem()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string();
                        let template = String::from_utf8(file.clone())?;

                        hbs.register_template_string(&name, template)?;
                    }
                    _ => continue,
                };
            }
        }

        Ok(Self { metadata, hbs })
    }

    pub fn from_dir(dir: Dir) -> AnyhowResult<Self> {
        let mut metadata: Vec<ResourceMetadata> = Vec::new();
        let mut hbs = handlebars::Handlebars::new();

        for file in dir.files() {
            match file.path().extension().and_then(|e| e.to_str()) {
                Some("json") => {
                    metadata.push(serde_json::from_slice(file.contents())?);
                }
                Some("hbs") => {
                    let name = file
                        .path()
                        .file_stem()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string();

                    let template = match file.contents_utf8() {
                        Some(template) => template.to_string(),
                        None => Default::default(),
                    };

                    hbs.register_template_string(&name, template)?;
                }
                _ => continue,
            };
        }

        Ok(Self { metadata, hbs })
    }
}

impl MetadataDriven for FileMetadata<'_> {
    fn find(&self) -> Result<Vec<ResourceMetadata>> {
        Ok(self.metadata.clone())
    }
    fn find_by_kind(&self, kind: &str) -> Result<Option<ResourceMetadata>> {
        Ok(self
            .metadata
            .clone()
            .into_iter()
            .find(|m| m.crd.spec.names.kind == kind))
    }

    fn render_hbs(&self, resource: &Resource) -> Result<String> {
        let value = serde_json::from_str(&resource.spec)
            .map_err(|_| Error::CommandMalformed("spec must be a json".into()))?;
        let mut data = match value {
            serde_json::Value::Object(v) => Ok(v),
            _ => Err(Error::CommandMalformed("invalid spec json".into())),
        }?;

        let Some(metadata) = self.find_by_kind(&resource.kind)? else {
            return Err(Error::Unexpected(format!(
                "metadata not found for {}",
                resource.kind
            )));
        };

        let tier = data
            .get("throughputTier")
            .map(|v| v.as_str().unwrap())
            .unwrap_or_default();

        if let Some(plan) = metadata.plan.get(tier) {
            data.insert("dns".into(), serde_json::Value::String(plan.dns.clone()));
        }
        data.insert(
            "name".into(),
            serde_json::Value::String(resource.name.clone()),
        );
        data.insert(
            "networkWithChainPrefix".into(),
            serde_json::Value::String(network_with_chain_prefix(
                data.get("network").unwrap().as_str().unwrap(),
            )),
        );

        let name = resource.kind.to_lowercase();
        let rendered = self.hbs.render(&name, &data)?;
        let value: serde_json::Value = serde_json::from_str(&rendered.replace('\n', ""))?;

        Ok(value.to_string())
    }
}
