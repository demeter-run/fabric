use dmtri::demeter::ops::v1alpha as proto;
use std::sync::Arc;
use tonic::{async_trait, Status};

use crate::domain::{
    auth::Credential,
    event::EventDrivenBridge,
    project::ProjectDrivenCache,
    resource::{self, CreateResourceCmd},
};

pub struct ResourceServiceImpl {
    pub project_cache: Arc<dyn ProjectDrivenCache>,
    pub event: Arc<dyn EventDrivenBridge>,
}
impl ResourceServiceImpl {
    pub fn new(
        project_cache: Arc<dyn ProjectDrivenCache>,
        event: Arc<dyn EventDrivenBridge>,
    ) -> Self {
        Self {
            project_cache,
            event,
        }
    }
}

#[async_trait]
impl proto::resource_service_server::ResourceService for ResourceServiceImpl {
    async fn create_resource(
        &self,
        request: tonic::Request<proto::CreateResourceRequest>,
    ) -> Result<tonic::Response<proto::CreateResourceResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::permission_denied("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = CreateResourceCmd::new(credential, req.project_id, req.kind, req.data);
        let result =
            resource::create(self.project_cache.clone(), self.event.clone(), cmd.clone()).await;

        if let Err(err) = result {
            return Err(Status::failed_precondition(err.to_string()));
        }

        let message = proto::CreateResourceResponse {
            id: cmd.id,
            kind: cmd.kind,
        };
        Ok(tonic::Response::new(message))
    }
}
