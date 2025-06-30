use dmtri::demeter::ops::v1alpha::{self as proto};
use std::sync::Arc;
use tonic::{async_trait, Status};

use crate::{
    domain::{
        auth::Credential,
        error::Error,
        project::cache::ProjectDrivenCache,
        resource::cache::ResourceDrivenCache,
        worker::{
            logs::{self, FetchDirection, Log, WorkerLogsDrivenStorage},
            signer::{self, Signer, WorkerSignerDrivenStorage},
            storage::{self, KeyValue, WorkerKeyValueDrivenStorage},
        },
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

        let cmd = storage::command::FetchCmd::new(
            credential,
            req.worker_id,
            req.key,
            req.page,
            req.page_size,
        )
        .inspect_err(|err| {
            handle_error_metric(self.metrics.clone(), "worker-key-value-storage", err)
        })?;

        let (count, values) = storage::command::fetch(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.worker_key_value_storage.clone(),
            cmd,
        )
        .await
        .inspect_err(|err| {
            handle_error_metric(self.metrics.clone(), "worker-key-value-storage", err)
        })?;

        let records = values.into_iter().map(|v| v.into()).collect();
        let message = proto::FetchKeyValueResponse {
            total_records: count as u32,
            records,
        };

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

        let cmd = storage::command::UpdateCmd::new(
            credential,
            KeyValue {
                worker_id: req.worker_id,
                key: req.key,
                value: req.value.into(),
            },
        );

        let value = storage::command::update(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.worker_key_value_storage.clone(),
            cmd,
        )
        .await
        .inspect_err(|err| {
            handle_error_metric(self.metrics.clone(), "worker-key-value-storage", err)
        })?;

        let message = proto::UpdateKeyValueResponse {
            updated: Some(value.into()),
        };

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

        let cmd = storage::command::DeleteCmd::new(credential, req.worker_id, req.key);

        storage::command::delete(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.worker_key_value_storage.clone(),
            cmd,
        )
        .await
        .inspect_err(|err| {
            handle_error_metric(self.metrics.clone(), "worker-key-value-storage", err)
        })?;

        let message = proto::DeleteKeyValueResponse {};

        Ok(tonic::Response::new(message))
    }
}

impl From<KeyValue> for proto::KeyValue {
    fn from(value: KeyValue) -> Self {
        Self {
            key: value.key,
            value: value.value.into(),
        }
    }
}

pub struct WorkerLogsServiceImpl {
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    logs_storage: Arc<dyn WorkerLogsDrivenStorage>,
    metrics: Arc<MetricsDriven>,
}
impl WorkerLogsServiceImpl {
    pub fn new(
        project_cache: Arc<dyn ProjectDrivenCache>,
        resource_cache: Arc<dyn ResourceDrivenCache>,
        logs_storage: Arc<dyn WorkerLogsDrivenStorage>,
        metrics: Arc<MetricsDriven>,
    ) -> Self {
        Self {
            project_cache,
            resource_cache,
            logs_storage,
            metrics,
        }
    }
}

#[async_trait]
impl proto::logs_service_server::LogsService for WorkerLogsServiceImpl {
    async fn fetch_window(
        &self,
        request: tonic::Request<proto::FetchLogsRequest>,
    ) -> Result<tonic::Response<proto::FetchLogsResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let direction = if let Some(direction) = req.direction {
            Some(direction.try_into()?)
        } else {
            None
        };

        let cmd = logs::command::FetchCmd::new(
            credential,
            req.worker_id,
            req.cursor as i64,
            direction,
            req.limit.map(|l| l as i64),
        )
        .inspect_err(|err| handle_error_metric(self.metrics.clone(), "worker-logs-storage", err))?;

        let logs = logs::command::fetch(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.logs_storage.clone(),
            cmd,
        )
        .await
        .inspect_err(|err| handle_error_metric(self.metrics.clone(), "worker-logs-storage", err))?;

        let records = logs.into_iter().map(|v| v.into()).collect();
        let message = proto::FetchLogsResponse { records };

        Ok(tonic::Response::new(message))
    }
}

impl TryFrom<i32> for FetchDirection {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Prev),
            1 => Ok(Self::Next),
            _ => Err(Error::CommandMalformed("invalid direction".into())),
        }
    }
}

impl From<Log> for proto::Log {
    fn from(value: Log) -> Self {
        Self {
            timestamp: value.timestamp.timestamp() as u32,
            level: value.level,
            message: value.message,
            context: value.context,
        }
    }
}

pub struct WorkerSignerServiceImpl {
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    signer_storage: Arc<dyn WorkerSignerDrivenStorage>,
    metrics: Arc<MetricsDriven>,
}
impl WorkerSignerServiceImpl {
    pub fn new(
        project_cache: Arc<dyn ProjectDrivenCache>,
        resource_cache: Arc<dyn ResourceDrivenCache>,
        signer_storage: Arc<dyn WorkerSignerDrivenStorage>,
        metrics: Arc<MetricsDriven>,
    ) -> Self {
        Self {
            project_cache,
            resource_cache,
            signer_storage,
            metrics,
        }
    }
}

#[async_trait]
impl proto::signer_service_server::SignerService for WorkerSignerServiceImpl {
    async fn list(
        &self,
        request: tonic::Request<proto::ListSignerRequest>,
    ) -> Result<tonic::Response<proto::ListSignerResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = signer::command::ListCmd::new(credential, req.worker_id);

        let signers = signer::command::list(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.signer_storage.clone(),
            cmd,
        )
        .await
        .inspect_err(|err| {
            handle_error_metric(self.metrics.clone(), "worker-signer-storage", err)
        })?;

        let keys = signers.into_iter().map(|v| v.into()).collect();
        let message = proto::ListSignerResponse { keys };

        Ok(tonic::Response::new(message))
    }

    async fn get_public_key(
        &self,
        request: tonic::Request<proto::GetPublicKeySignerRequest>,
    ) -> Result<tonic::Response<proto::GetPublicKeySignerResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = signer::command::GetPublicKeyCmd::new(credential, req.worker_id, req.key_name);

        let public_key = signer::command::get_public_key(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.signer_storage.clone(),
            cmd,
        )
        .await
        .inspect_err(|err| {
            handle_error_metric(self.metrics.clone(), "worker-signer-storage", err)
        })?;

        let message = proto::GetPublicKeySignerResponse {
            public_key: public_key.map(|x| x.into()),
        };

        Ok(tonic::Response::new(message))
    }

    async fn sign_payload(
        &self,
        request: tonic::Request<proto::SignPayloadSignerRequest>,
    ) -> Result<tonic::Response<proto::SignPayloadSignerResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = signer::command::SignPayloadCmd::new(
            credential,
            req.worker_id,
            req.key_name,
            req.payload.to_vec(),
        );

        let signed_payload = signer::command::sign_payload(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.signer_storage.clone(),
            cmd,
        )
        .await
        .inspect_err(|err| {
            handle_error_metric(self.metrics.clone(), "worker-signer-storage", err)
        })?;

        let message = proto::SignPayloadSignerResponse {
            signed_payload: signed_payload.into(),
        };

        Ok(tonic::Response::new(message))
    }
}

impl From<Signer> for proto::Signer {
    fn from(value: Signer) -> Self {
        Self {
            key_name: value.key_name,
        }
    }
}
