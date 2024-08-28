use dmtri::demeter::ops::v1alpha as proto;
use std::sync::Arc;
use tonic::{async_trait, Status};
use tracing::error;
use uuid::Uuid;

use crate::domain::{
    auth::{Auth0Driven, Credential},
    event::EventDrivenBridge,
    project::{self, cache::ProjectDrivenCache, Project, StripeDriven},
};

pub struct ProjectServiceImpl {
    pub cache: Arc<dyn ProjectDrivenCache>,
    pub event: Arc<dyn EventDrivenBridge>,
    pub auth0: Arc<dyn Auth0Driven>,
    pub stripe: Arc<dyn StripeDriven>,
    pub secret: String,
}

impl ProjectServiceImpl {
    pub fn new(
        cache: Arc<dyn ProjectDrivenCache>,
        event: Arc<dyn EventDrivenBridge>,
        auth0: Arc<dyn Auth0Driven>,
        stripe: Arc<dyn StripeDriven>,
        secret: String,
    ) -> Self {
        Self {
            cache,
            event,
            auth0,
            stripe,
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

        project::command::create(
            self.cache.clone(),
            self.event.clone(),
            self.auth0.clone(),
            self.stripe.clone(),
            cmd.clone(),
        )
        .await?;

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
    async fn fetch_project_secrets(
        &self,
        request: tonic::Request<proto::FetchProjectSecretsRequest>,
    ) -> Result<tonic::Response<proto::FetchProjectSecretsResponse>, tonic::Status> {
        let _credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let message = proto::FetchProjectSecretsResponse {
            records: vec![proto::ProjectSecret {
                id: Uuid::new_v4().to_string(),
                name: "Secret Name".into(),
                project_id: req.project_id,
                ..Default::default()
            }],
        };

        Ok(tonic::Response::new(message))
    }

    async fn create_project_payment(
        &self,
        request: tonic::Request<proto::CreateProjectPaymentRequest>,
    ) -> Result<tonic::Response<proto::CreateProjectPaymentResponse>, tonic::Status> {
        let _credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let message = proto::CreateProjectPaymentResponse {
            id: Uuid::new_v4().to_string(),
            project_id: req.project_id,
            provider: "stripe".into(),
            provider_id: "provider id".into(),
            subscription_id: Some("subscription id".into()),
        };

        Ok(tonic::Response::new(message))
    }
    async fn fetch_project_payment(
        &self,
        request: tonic::Request<proto::FetchProjectPaymentRequest>,
    ) -> Result<tonic::Response<proto::FetchProjectPaymentResponse>, tonic::Status> {
        let _credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let message = proto::FetchProjectPaymentResponse {
            records: vec![proto::ProjectPayment {
                id: Uuid::new_v4().to_string(),
                project_id: req.project_id,
                provider: "stripe".into(),
                provider_id: "provider id".into(),
                subscription_id: Some("subscription id".into()),
                ..Default::default()
            }],
        };

        Ok(tonic::Response::new(message))
    }

    async fn create_project_invite(
        &self,
        request: tonic::Request<proto::CreateProjectInviteRequest>,
    ) -> Result<tonic::Response<proto::CreateProjectInviteResponse>, tonic::Status> {
        let _credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let _req = request.into_inner();

        let message = proto::CreateProjectInviteResponse {};

        Ok(tonic::Response::new(message))
    }
    async fn fetch_project_users(
        &self,
        request: tonic::Request<proto::FetchProjectUsersRequest>,
    ) -> Result<tonic::Response<proto::FetchProjectUsersResponse>, tonic::Status> {
        let _credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let message = proto::FetchProjectUsersResponse {
            records: vec![proto::ProjectUser {
                id: Uuid::new_v4().to_string(),
                project_id: req.project_id,
                user_id: "auth0 id".into(),
                role: "owner".into(),
                ..Default::default()
            }],
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
