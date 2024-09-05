use std::sync::Arc;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;

use super::{MetadataDriven, Result};

pub async fn fetch(metadata: Arc<dyn MetadataDriven>) -> Result<Vec<CustomResourceDefinition>> {
    let crds = metadata.find().await?;
    Ok(crds)
}

#[cfg(test)]
mod tests {
    use crate::domain::metadata::MockMetadataDriven;

    use super::*;

    #[tokio::test]
    async fn it_should_fetch_metadata() {
        let mut metadata = MockMetadataDriven::new();
        metadata
            .expect_find()
            .return_once(|| Ok(vec![CustomResourceDefinition::default()]));

        let result = fetch(Arc::new(metadata)).await;
        assert!(result.is_ok());
    }
}
