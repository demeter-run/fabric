use dmtri::demeter::ops::v1alpha::{self as proto, DeleteResourceResponse};
use std::sync::Arc;
use tonic::{async_trait, Status};

use crate::domain::{
    auth::Credential,
    event::EventDrivenBridge,
    metadata::MetadataDriven,
    project::cache::ProjectDrivenCache,
    resource::{cache::ResourceDrivenCache, command, Resource},
};

pub struct ResourceServiceImpl {
    pub project_cache: Arc<dyn ProjectDrivenCache>,
    pub resource_cache: Arc<dyn ResourceDrivenCache>,
    pub event: Arc<dyn EventDrivenBridge>,
    pub metadata: Arc<dyn MetadataDriven>,
}
impl ResourceServiceImpl {
    pub fn new(
        project_cache: Arc<dyn ProjectDrivenCache>,
        resource_cache: Arc<dyn ResourceDrivenCache>,
        event: Arc<dyn EventDrivenBridge>,
        metadata: Arc<dyn MetadataDriven>,
    ) -> Self {
        Self {
            project_cache,
            resource_cache,
            event,
            metadata,
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

        let cmd = command::FetchCmd::new(credential, req.project_id, req.page, req.page_size)?;

        let resources = command::fetch(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.metadata.clone(),
            cmd,
        )
        .await?;

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
        .await?;

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

        let value = serde_json::from_str(&req.spec)
            .map_err(|_| Status::failed_precondition("spec must be a json"))?;
        let spec = match value {
            serde_json::Value::Object(v) => Ok(v),
            _ => Err(Status::failed_precondition("invalid spec json")),
        }?;

        let cmd = command::CreateCmd::new(credential, req.project_id, req.kind, spec);

        command::create(
            self.project_cache.clone(),
            self.metadata.clone(),
            self.event.clone(),
            cmd.clone(),
        )
        .await?;

        let message = proto::CreateResourceResponse {
            id: cmd.id,
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

        let value = serde_json::from_str(&req.spec_patch)
            .map_err(|_| Status::failed_precondition("spec must be a json"))?;
        let spec = match value {
            serde_json::Value::Object(v) => Ok(v),
            _ => Err(Status::failed_precondition("invalid spec json")),
        }?;

        let cmd = command::UpdateCmd::new(credential, req.id, spec);

        let updated = command::update(
            self.project_cache.clone(),
            self.resource_cache.clone(),
            self.event.clone(),
            cmd.clone(),
        )
        .await?;

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
        .await?;

        Ok(tonic::Response::new(DeleteResourceResponse {}))
    }
}

impl From<Resource> for proto::Resource {
    fn from(value: Resource) -> Self {
        Self {
            id: value.id,
            kind: value.kind,
            spec: value.spec,
            annotations: value.annotations,
            status: value.status.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}
