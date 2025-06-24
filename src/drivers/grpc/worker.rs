use dmtri::demeter::ops::v1alpha::{self as proto};
use std::sync::Arc;
use tonic::{async_trait, Status};

use crate::{
    domain::{
        auth::Credential,
        project::cache::ProjectDrivenCache,
        resource::cache::ResourceDrivenCache,
        worker::{command, KeyValue, WorkerKeyValueDrivenStorage},
    },
    driven::prometheus::metrics::MetricsDriven,
};

use super::handle_error_metric;

pub struct WorkerKeyValueServiceImpl {
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    worker_key_value_storage: Arc<dyn WorkerKeyValueDrivenStorage>,
    metrics: Arc<MetricsDriven>,
}
impl WorkerKeyValueServiceImpl {
    pub fn new(
        project_cache: Arc<dyn ProjectDrivenCache>,
        resource_cache: Arc<dyn ResourceDrivenCache>,
        worker_key_value_storage: Arc<dyn WorkerKeyValueDrivenStorage>,
        metrics: Arc<MetricsDriven>,
    ) -> Self {
        Self {
            project_cache,
            resource_cache,
            worker_key_value_storage,
            metrics,
        }
    }
}

#[async_trait]
impl proto::key_value_service_server::KeyValueService for WorkerKeyValueServiceImpl {
    async fn fetch_key_value(
        &self,
        request: tonic::Request<proto::FetchKeyValueRequest>,
    ) -> Result<tonic::Response<proto::FetchKeyValueResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd =
            command::FetchCmd::new(credential, req.worker_id, req.key, req.page, req.page_size)
                .inspect_err(|err| handle_error_metric(self.metrics.clone(), "worker", err))?;

        let values = command::fetch(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.worker_key_value_storage.clone(),
            cmd,
        )
        .await
        .inspect_err(|err| handle_error_metric(self.metrics.clone(), "worker", err))?;

        let records = values.into_iter().map(|v| v.into()).collect();
        let message = proto::FetchKeyValueResponse { records };

        Ok(tonic::Response::new(message))
    }

    async fn update_key_value(
        &self,
        request: tonic::Request<proto::UpdateKeyValueRequest>,
    ) -> Result<tonic::Response<proto::UpdateKeyValueResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = command::UpdateCmd::new(
            credential,
            KeyValue {
                worker_id: req.worker_id,
                key: req.key,
                value: req.value.into(),
                r#type: req
                    .r#type
                    .parse()
                    .inspect_err(|err| handle_error_metric(self.metrics.clone(), "worker", err))?,
                secure: req.secure,
            },
        );

        command::update(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.worker_key_value_storage.clone(),
            cmd,
        )
        .await
        .inspect_err(|err| handle_error_metric(self.metrics.clone(), "worker", err))?;

        let message = proto::UpdateKeyValueResponse {};

        Ok(tonic::Response::new(message))
    }

    async fn delete_key_value(
        &self,
        request: tonic::Request<proto::DeleteKeyValueRequest>,
    ) -> Result<tonic::Response<proto::DeleteKeyValueResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = command::DeleteCmd::new(credential, req.worker_id, req.key);

        command::delete(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.worker_key_value_storage.clone(),
            cmd,
        )
        .await
        .inspect_err(|err| handle_error_metric(self.metrics.clone(), "worker", err))?;

        let message = proto::DeleteKeyValueResponse {};

        Ok(tonic::Response::new(message))
    }
}

impl From<KeyValue> for proto::KeyValue {
    fn from(value: KeyValue) -> Self {
        Self {
            key: value.key,
            value: value.value.into(),
            r#type: value.r#type.to_string(),
            secure: value.secure,
        }
    }
}
