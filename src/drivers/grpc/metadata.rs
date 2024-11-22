use std::sync::Arc;

use dmtri::demeter::ops::v1alpha as proto;
use tonic::async_trait;
use tracing::error;

use crate::{
    domain::{
        error::Error,
        metadata::{self, MetadataDriven},
    },
    driven::prometheus::metrics::MetricsDriven,
};

use super::handle_error_metric;

pub struct MetadataServiceImpl {
    metadata: Arc<dyn MetadataDriven>,
    metrics: Arc<MetricsDriven>,
}
impl MetadataServiceImpl {
    pub fn new(metadata: Arc<dyn MetadataDriven>, metrics: Arc<MetricsDriven>) -> Self {
        Self { metadata, metrics }
    }
}

#[async_trait]
impl proto::metadata_service_server::MetadataService for MetadataServiceImpl {
    async fn fetch_metadata(
        &self,
        _request: tonic::Request<proto::FetchMetadataRequest>,
    ) -> Result<tonic::Response<proto::FetchMetadataResponse>, tonic::Status> {
        let metadata = metadata::command::fetch(self.metadata.clone())
            .await
            .inspect_err(|err| handle_error_metric(self.metrics.clone(), "metadata", err))?;

        let records: Vec<proto::Metadata> = metadata
            .iter()
            .map(|m| {
                Ok(proto::Metadata {
                    options: serde_json::to_string(&m.options)?,
                    crd: serde_json::to_string(&m.crd)?,
                })
            })
            .collect::<Result<_, _>>()
            .map_err(|err: Error| {
                error!(?err, "error to map metadata");
                tonic::Status::internal(err.to_string())
            })?;

        let message = proto::FetchMetadataResponse { records };

        Ok(tonic::Response::new(message))
    }
}
