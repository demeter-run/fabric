use std::{fs, path::Path};

use anyhow::Result as AnyhowResult;

use crate::domain::{
    metadata::{MetadataDriven, ResourceMetadata},
    Result,
};

pub struct FileMetadata<'a> {
    metadata: Vec<ResourceMetadata>,
    hbs: handlebars::Handlebars<'a>,
}

impl<'a> FileMetadata<'a> {
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
}

#[async_trait::async_trait]
impl<'a> MetadataDriven for FileMetadata<'a> {
    async fn find(&self) -> Result<Vec<ResourceMetadata>> {
        Ok(self.metadata.clone())
    }
    async fn find_by_kind(&self, kind: &str) -> Result<Option<ResourceMetadata>> {
        Ok(self
            .metadata
            .clone()
            .into_iter()
            .find(|m| m.crd.spec.names.kind == kind))
    }

    fn render_hbs(&self, name: &str, spec: &str) -> Result<String> {
        let data: serde_json::Value = serde_json::from_str(spec)?;
        let rendered = self.hbs.render(name, &data)?;
        let value: serde_json::Value = serde_json::from_str(&rendered.replace('\n', ""))?;

        Ok(value.to_string())
    }
}
