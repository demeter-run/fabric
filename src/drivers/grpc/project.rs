use dmtri::demeter::ops::v1alpha as proto;
use std::sync::Arc;
use tonic::{async_trait, Status};

use crate::domain::{
    auth::Credential,
    event::EventDrivenBridge,
    project::{
        self, CreateProjectCmd, CreateProjectSecretCmd, FindProjectCmd, ProjectCache,
        ProjectDrivenCache,
    },
};

pub struct ProjectServiceImpl {
    pub cache: Arc<dyn ProjectDrivenCache>,
    pub event: Arc<dyn EventDrivenBridge>,
    pub secret: String,
}

impl ProjectServiceImpl {
    pub fn new(
        cache: Arc<dyn ProjectDrivenCache>,
        event: Arc<dyn EventDrivenBridge>,
        secret: String,
    ) -> Self {
        Self {
            cache,
            event,
            secret,
        }
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

        let cmd = CreateProjectCmd::new(credential, req.name);

        let result = project::create(self.cache.clone(), self.event.clone(), cmd.clone()).await;
        if let Err(err) = result {
            return Err(Status::failed_precondition(err.to_string()));
        }

        let message = proto::CreateProjectResponse {
            id: cmd.id,
            name: cmd.name,
            namespace: cmd.namespace,
        };

        Ok(tonic::Response::new(message))
    }

    async fn create_project_secret(
        &self,
        request: tonic::Request<proto::CreateProjectSecretRequest>,
    ) -> Result<tonic::Response<proto::CreateProjectSecretResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::permission_denied("invalid credential")),
        };

        let req = request.into_inner();

        let cmd =
            CreateProjectSecretCmd::new(credential, self.secret.clone(), req.project_id, req.name);

        let result =
            project::create_secret(self.cache.clone(), self.event.clone(), cmd.clone()).await;
        if let Err(err) = result {
            return Err(Status::failed_precondition(err.to_string()));
        }

        let key = result.unwrap();
        let message = proto::CreateProjectSecretResponse {
            id: cmd.id,
            name: cmd.name,
            key,
        };

        Ok(tonic::Response::new(message))
    }

    async fn find_projects(
        &self,
        request: tonic::Request<proto::FindProjectsRequest>,
    ) -> Result<tonic::Response<proto::FindProjectsResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::permission_denied("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = FindProjectCmd::new(credential, req.page, req.page_size)
            .map_err(|err| Status::failed_precondition(err.to_string()))?;

        let projects = project::find_cache(self.cache.clone(), cmd.clone())
            .await
            .map_err(|err| Status::failed_precondition(err.to_string()))?;

        let records = projects.into_iter().map(|v| v.into()).collect();
        let message = proto::FindProjectsResponse { records };

        Ok(tonic::Response::new(message))
    }
}

impl From<ProjectCache> for proto::Project {
    fn from(value: ProjectCache) -> Self {
        Self {
            id: value.id,
            name: value.name,
            status: value.status.to_string(),
            namespace: value.namespace,
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}
