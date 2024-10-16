use std::sync::Arc;

use super::{MetadataDriven, ResourceMetadata, Result};

pub async fn fetch(metadata: Arc<dyn MetadataDriven>) -> Result<Vec<ResourceMetadata>> {
    metadata.find()
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
            .return_once(|| Ok(vec![ResourceMetadata::default()]));

        let result = fetch(Arc::new(metadata)).await;
        assert!(result.is_ok());
    }
}
