use dmtri::demeter::ops::v1alpha as proto;
use std::sync::Arc;
use tonic::{async_trait, Status};

use crate::domain::{
    events::EventBridge,
    ports::{self, Port},
    projects::ProjectCache,
};

pub struct PortServiceImpl {
    pub project_cache: Arc<dyn ProjectCache>,
    pub event: Arc<dyn EventBridge>,
}
impl PortServiceImpl {
    pub fn new(project_cache: Arc<dyn ProjectCache>, event: Arc<dyn EventBridge>) -> Self {
        Self {
            project_cache,
            event,
        }
    }
}

#[async_trait]
impl proto::port_service_server::PortService for PortServiceImpl {
    async fn create_port(
        &self,
        request: tonic::Request<proto::CreatePortRequest>,
    ) -> Result<tonic::Response<proto::CreatePortResponse>, tonic::Status> {
        let req = request.into_inner();

        let port = Port::new(&req.project_id, &req.kind, &req.data);
        let result =
            ports::create::create(self.project_cache.clone(), self.event.clone(), port.clone())
                .await;

        if let Err(err) = result {
            return Err(Status::failed_precondition(err.to_string()));
        }

        let message = proto::CreatePortResponse {
            id: port.id,
            kind: port.kind,
        };
        Ok(tonic::Response::new(message))
    }
}
