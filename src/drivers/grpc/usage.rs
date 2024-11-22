use dmtri::demeter::ops::v1alpha::{self as proto};
use std::sync::Arc;
use tonic::{async_trait, Status};

use crate::{
    domain::{
        auth::Credential,
        metadata::MetadataDriven,
        project::cache::ProjectDrivenCache,
        usage::{cache::UsageDrivenCache, command, UsageReport},
    },
    driven::prometheus::metrics::MetricsDriven,
};

use super::handle_error_metric;

pub struct UsageServiceImpl {
    project_cache: Arc<dyn ProjectDrivenCache>,
    usage_cache: Arc<dyn UsageDrivenCache>,
    metadata: Arc<dyn MetadataDriven>,
    metrics: Arc<MetricsDriven>,
}
impl UsageServiceImpl {
    pub fn new(
        project_cache: Arc<dyn ProjectDrivenCache>,
        usage_cache: Arc<dyn UsageDrivenCache>,
        metadata: Arc<dyn MetadataDriven>,
        metrics: Arc<MetricsDriven>,
    ) -> Self {
        Self {
            project_cache,
            usage_cache,
            metadata,
            metrics,
        }
    }
}

#[async_trait]
impl proto::usage_service_server::UsageService for UsageServiceImpl {
    async fn fetch_usage_report(
        &self,
        request: tonic::Request<proto::FetchUsageReportRequest>,
    ) -> Result<tonic::Response<proto::FetchUsageReportResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = command::FetchCmd::new(credential, req.project_id, req.page, req.page_size)
            .inspect_err(|err| handle_error_metric(self.metrics.clone(), "usage", err))?;

        let usage_report = command::fetch_report(
            self.project_cache.clone(),
            self.usage_cache.clone(),
            self.metadata.clone(),
            cmd,
        )
        .await
        .inspect_err(|err| handle_error_metric(self.metrics.clone(), "usage", err))?;

        let records = usage_report.into_iter().map(|v| v.into()).collect();
        let message = proto::FetchUsageReportResponse { records };

        Ok(tonic::Response::new(message))
    }
}

impl From<UsageReport> for proto::UsageReport {
    fn from(value: UsageReport) -> Self {
        Self {
            resource_id: value.resource_id,
            resource_kind: value.resource_kind,
            resource_name: value.resource_name,
            resource_spec: value.resource_spec,
            units: value.units,
            tier: value.tier,
            period: value.period,
            units_cost: value.units_cost,
            minimum_cost: value.minimum_cost,
        }
    }
}
