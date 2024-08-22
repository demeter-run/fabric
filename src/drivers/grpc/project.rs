use dmtri::demeter::ops::v1alpha as proto;
use std::sync::Arc;
use tonic::{async_trait, Status};
use tracing::error;

use crate::domain::{
    auth::Credential,
    event::EventDrivenBridge,
    project::{self, cache::ProjectDrivenCache, Project},
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
    async fn fetch_projects(
        &self,
        request: tonic::Request<proto::FetchProjectsRequest>,
    ) -> Result<tonic::Response<proto::FetchProjectsResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = project::command::FetchCmd::new(credential, req.page, req.page_size)?;

        let projects = project::command::fetch(self.cache.clone(), cmd.clone()).await?;

        let records = projects.into_iter().map(|v| v.into()).collect();
        let message = proto::FetchProjectsResponse { records };

        Ok(tonic::Response::new(message))
    }

    async fn create_project(
        &self,
        request: tonic::Request<proto::CreateProjectRequest>,
    ) -> Result<tonic::Response<proto::CreateProjectResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = project::command::CreateCmd::new(credential, req.name);

        project::command::create(self.cache.clone(), self.event.clone(), cmd.clone()).await?;

        let message = proto::CreateProjectResponse {
            id: cmd.id,
            name: cmd.name,
            namespace: cmd.namespace,
        };

        Ok(tonic::Response::new(message))
    }

    async fn update_project(
        &self,
        request: tonic::Request<proto::UpdateProjectRequest>,
    ) -> Result<tonic::Response<proto::UpdateProjectResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();
        let cmd = project::command::UpdateCmd::new(credential, req.id, req.name);
        let updated =
            match project::command::update(self.cache.clone(), self.event.clone(), cmd.clone())
                .await
            {
                Ok(project) => project,
                Err(err) => {
                    error!(
                        error = err.to_string(),
                        "Unexpected error while performing project update."
                    );
                    return Err(Status::internal("Error running update."));
                }
            };
        let message = proto::UpdateProjectResponse {
            updated: Some(updated.into()),
        };

        Ok(tonic::Response::new(message))
    }

    async fn delete_project(
        &self,
        request: tonic::Request<proto::DeleteProjectRequest>,
    ) -> Result<tonic::Response<proto::DeleteProjectResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();
        let cmd = project::command::DeleteCmd::new(credential, req.id);
        project::command::delete(self.cache.clone(), self.event.clone(), cmd.clone()).await?;
        let message = proto::DeleteProjectResponse {};

        Ok(tonic::Response::new(message))
    }

    async fn create_project_secret(
        &self,
        request: tonic::Request<proto::CreateProjectSecretRequest>,
    ) -> Result<tonic::Response<proto::CreateProjectSecretResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = project::command::CreateSecretCmd::new(
            credential,
            self.secret.clone(),
            req.project_id,
            req.name,
        );

        let key =
            project::command::create_secret(self.cache.clone(), self.event.clone(), cmd.clone())
                .await?;

        let message = proto::CreateProjectSecretResponse {
            id: cmd.id,
            name: cmd.name,
            key,
        };

        Ok(tonic::Response::new(message))
    }
}

impl From<Project> for proto::Project {
    fn from(value: Project) -> Self {
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
