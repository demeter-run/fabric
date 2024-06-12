use rand::{distributions::Alphanumeric, Rng};
use std::sync::Arc;
use tonic::{async_trait, Request, Response, Status};

use crate::domain::management::{
    self,
    events::EventBridge,
    project::{Project, ProjectCache},
};

use super::proto::project::{
    project_service_server::ProjectService, CreateProjectRequest, CreateProjectResponse,
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
impl ProjectService for ProjectServiceImpl {
    async fn create_project(
        &self,
        request: Request<CreateProjectRequest>,
    ) -> Result<Response<CreateProjectResponse>, Status> {
        let req = request.into_inner();

        let slug: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();

        let project = Project {
            name: req.name,
            slug,
        };

        let result =
            management::project::create(self.cache.clone(), self.event.clone(), project.clone())
                .await;

        if let Err(err) = result {
            return Err(Status::failed_precondition(err.to_string()));
        }

        let message = CreateProjectResponse {
            name: project.name,
            slug: project.slug,
        };
        Ok(Response::new(message))
    }
}
