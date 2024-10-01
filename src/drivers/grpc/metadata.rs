use std::sync::Arc;

use dmtri::demeter::ops::v1alpha as proto;
use tonic::async_trait;

use crate::domain::{
    error::Error,
    metadata::{self, MetadataDriven},
};

pub struct MetadataServiceImpl {
    pub metadata: Arc<dyn MetadataDriven>,
}
impl MetadataServiceImpl {
    pub fn new(metadata: Arc<dyn MetadataDriven>) -> Self {
        Self { metadata }
    }
}

#[async_trait]
impl proto::metadata_service_server::MetadataService for MetadataServiceImpl {
    async fn fetch_metadata(
        &self,
        _request: tonic::Request<proto::FetchMetadataRequest>,
    ) -> Result<tonic::Response<proto::FetchMetadataResponse>, tonic::Status> {
        let metadata = metadata::command::fetch(self.metadata.clone()).await?;

        let records: Vec<proto::Metadata> = metadata
            .iter()
            .map(|m| {
                Ok(proto::Metadata {
                    options: serde_json::to_string(&m.options)?,
                    crd: serde_json::to_string(&m.crd)?,
                })
            })
            .collect::<Result<_, _>>()
            .map_err(|err: Error| tonic::Status::internal(err.to_string()))?;

        let message = proto::FetchMetadataResponse { records };

        Ok(tonic::Response::new(message))
    }
}
