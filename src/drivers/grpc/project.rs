use dmtri::demeter::ops::v1alpha as proto;
use std::sync::Arc;
use tonic::{async_trait, Status};

use crate::domain::{
    events::EventBridge,
    projects::{self, Project, ProjectCache},
    users::Credential,
};

pub struct ProjectServiceImpl {
    pub cache: Arc<dyn ProjectCache>,
    pub event: Arc<dyn EventBridge>,
}

impl ProjectServiceImpl {
    pub fn new(cache: Arc<dyn ProjectCache>, event: Arc<dyn EventBridge>) -> Self {
        Self { cache, event }
    }
}

#[async_trait]
impl proto::project_service_server::ProjectService for ProjectServiceImpl {
    async fn create_project(
        &self,
        request: tonic::Request<proto::CreateProjectRequest>,
    ) -> Result<tonic::Response<proto::CreateProjectResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::permission_denied("invalid credential")),
        };

        let req = request.into_inner();

        let project = Project::new(req.name, credential.id);
        let result =
            projects::create::create(self.cache.clone(), self.event.clone(), project.clone()).await;

        if let Err(err) = result {
            return Err(Status::failed_precondition(err.to_string()));
        }

        let message = proto::CreateProjectResponse {
            id: project.id,
            name: project.name,
            namespace: project.namespace,
        };
        Ok(tonic::Response::new(message))
    }
}
