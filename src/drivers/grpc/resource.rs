use dmtri::demeter::ops::v1alpha::{self as proto, DeleteResourceResponse};
use std::sync::Arc;
use tonic::{async_trait, Status};

use crate::{
    domain::{
        auth::Credential,
        event::EventDrivenBridge,
        metadata::MetadataDriven,
        project::cache::ProjectDrivenCache,
        resource::{cache::ResourceDrivenCache, command, Resource},
    },
    driven::prometheus::metrics::MetricsDriven,
};

use super::handle_error_metric;

pub struct ResourceServiceImpl {
    project_cache: Arc<dyn ProjectDrivenCache>,
    resource_cache: Arc<dyn ResourceDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    metadata: Arc<dyn MetadataDriven>,
    metrics: Arc<MetricsDriven>,
}
impl ResourceServiceImpl {
    pub fn new(
        project_cache: Arc<dyn ProjectDrivenCache>,
        resource_cache: Arc<dyn ResourceDrivenCache>,
        event: Arc<dyn EventDrivenBridge>,
        metadata: Arc<dyn MetadataDriven>,
        metrics: Arc<MetricsDriven>,
    ) -> Self {
        Self {
            project_cache,
            resource_cache,
            event,
            metadata,
            metrics,
        }
    }
}

#[async_trait]
impl proto::resource_service_server::ResourceService for ResourceServiceImpl {
    async fn fetch_resources(
        &self,
        request: tonic::Request<proto::FetchResourcesRequest>,
    ) -> Result<tonic::Response<proto::FetchResourcesResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = command::FetchCmd::new(credential, req.project_id, req.page, req.page_size)
            .map_err(|err| {
                handle_error_metric(self.metrics.clone(), "resource", &err);
                err
            })?;

        let resources = command::fetch(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.metadata.clone(),
            cmd,
        )
        .await
        .map_err(|err| {
            handle_error_metric(self.metrics.clone(), "resource", &err);
            err
        })?;

        let records = resources.into_iter().map(|v| v.into()).collect();
        let message = proto::FetchResourcesResponse { records };

        Ok(tonic::Response::new(message))
    }
    async fn fetch_resources_by_id(
        &self,
        request: tonic::Request<proto::FetchResourcesByIdRequest>,
    ) -> Result<tonic::Response<proto::FetchResourcesByIdResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = command::FetchByIdCmd {
            credential,
            id: req.id,
        };

        let resource = command::fetch_by_id(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.metadata.clone(),
            cmd,
        )
        .await
        .map_err(|err| {
            handle_error_metric(self.metrics.clone(), "resource", &err);
            err
        })?;

        let records = vec![resource.into()];
        let message = proto::FetchResourcesByIdResponse { records };

        Ok(tonic::Response::new(message))
    }

    async fn create_resource(
        &self,
        request: tonic::Request<proto::CreateResourceRequest>,
    ) -> Result<tonic::Response<proto::CreateResourceResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = command::CreateCmd::new(credential, req.project_id, req.kind, req.spec).map_err(
            |err| {
                handle_error_metric(self.metrics.clone(), "resource", &err);
                err
            },
        )?;

        command::create(
            self.resource_cache.clone(),
            self.project_cache.clone(),
            self.metadata.clone(),
            self.event.clone(),
            cmd.clone(),
        )
        .await
        .map_err(|err| {
            handle_error_metric(self.metrics.clone(), "resource", &err);
            err
        })?;

        let message = proto::CreateResourceResponse {
            id: cmd.id,
            name: cmd.name,
            kind: cmd.kind,
        };

        Ok(tonic::Response::new(message))
    }

    async fn update_resource(
        &self,
        request: tonic::Request<proto::UpdateResourceRequest>,
    ) -> Result<tonic::Response<proto::UpdateResourceResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = command::UpdateCmd::new(credential, req.id, req.spec_patch).map_err(|err| {
            handle_error_metric(self.metrics.clone(), "resource", &err);
            err
        })?;

        let updated = command::update(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.event.clone(),
            cmd.clone(),
        )
        .await
        .map_err(|err| {
            handle_error_metric(self.metrics.clone(), "resource", &err);
            err
        })?;

        let message = proto::UpdateResourceResponse {
            updated: Some(updated.into()),
        };

        Ok(tonic::Response::new(message))
    }

    async fn delete_resource(
        &self,
        request: tonic::Request<proto::DeleteResourceRequest>,
    ) -> Result<tonic::Response<proto::DeleteResourceResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = command::DeleteCmd {
            credential,
            id: req.id,
        };

        command::delete(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.event.clone(),
            cmd,
        )
        .await
        .map_err(|err| {
            handle_error_metric(self.metrics.clone(), "resource", &err);
            err
        })?;

        Ok(tonic::Response::new(DeleteResourceResponse {}))
    }
}

impl From<Resource> for proto::Resource {
    fn from(value: Resource) -> Self {
        Self {
            id: value.id,
            name: value.name,
            kind: value.kind,
            spec: value.spec,
            annotations: value.annotations,
            status: value.status.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}
