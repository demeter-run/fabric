use dmtri::demeter::ops::v1alpha as proto;
use std::{sync::Arc, time::Duration};
use tonic::{async_trait, Status};
use tracing::error;

use crate::domain::{
    auth::{Auth0Driven, Credential},
    event::EventDrivenBridge,
    project::{
        self, cache::ProjectDrivenCache, Project, ProjectEmailDriven, ProjectSecret,
        ProjectUserAggregated, ProjectUserInvite, StripeDriven,
    },
};

pub struct ProjectServiceImpl {
    pub cache: Arc<dyn ProjectDrivenCache>,
    pub event: Arc<dyn EventDrivenBridge>,
    pub auth0: Arc<dyn Auth0Driven>,
    pub stripe: Arc<dyn StripeDriven>,
    pub email: Arc<dyn ProjectEmailDriven>,
    pub secret: String,
    pub invite_ttl: Duration,
}

impl ProjectServiceImpl {
    pub fn new(
        cache: Arc<dyn ProjectDrivenCache>,
        event: Arc<dyn EventDrivenBridge>,
        auth0: Arc<dyn Auth0Driven>,
        stripe: Arc<dyn StripeDriven>,
        email: Arc<dyn ProjectEmailDriven>,
        secret: String,
        invite_ttl: Duration,
    ) -> Self {
        Self {
            cache,
            event,
            auth0,
            stripe,
            email,
            secret,
            invite_ttl,
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
    async fn fetch_project_by_namespace(
        &self,
        request: tonic::Request<proto::FetchProjectByNamespaceRequest>,
    ) -> Result<tonic::Response<proto::FetchProjectByNamespaceResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = project::command::FetchByNamespaceCmd::new(credential, req.namespace);

        let project = project::command::fetch_by_namespace(self.cache.clone(), cmd.clone()).await?;

        let message = proto::FetchProjectByNamespaceResponse {
            records: vec![project.into()],
        };

        Ok(tonic::Response::new(message))
    }
    async fn fetch_project_by_id(
        &self,
        request: tonic::Request<proto::FetchProjectByIdRequest>,
    ) -> Result<tonic::Response<proto::FetchProjectByIdResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = project::command::FetchByIdCmd::new(credential, req.id);

        let project = project::command::fetch_by_id(self.cache.clone(), cmd.clone()).await?;

        let message = proto::FetchProjectByIdResponse {
            records: vec![project.into()],
        };

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
    async fn fetch_project_secrets(
        &self,
        request: tonic::Request<proto::FetchProjectSecretsRequest>,
    ) -> Result<tonic::Response<proto::FetchProjectSecretsResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = project::command::FetchSecretCmd::new(credential, req.project_id);

        let secrets = project::command::fetch_secret(self.cache.clone(), cmd.clone()).await?;

        let records = secrets.into_iter().map(|v| v.into()).collect();
        let message = proto::FetchProjectSecretsResponse { records };

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
    async fn delete_project_secret(
        &self,
        request: tonic::Request<proto::DeleteProjectSecretRequest>,
    ) -> Result<tonic::Response<proto::DeleteProjectSecretResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();
        let cmd = project::command::DeleteSecretCmd::new(credential, req.id);
        project::command::delete_secret(self.cache.clone(), self.event.clone(), cmd.clone())
            .await?;
        let message = proto::DeleteProjectSecretResponse {};

        Ok(tonic::Response::new(message))
    }
    async fn fetch_project_users(
        &self,
        request: tonic::Request<proto::FetchProjectUsersRequest>,
    ) -> Result<tonic::Response<proto::FetchProjectUsersResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = project::command::FetchUserCmd::new(
            credential,
            req.page,
            req.page_size,
            req.project_id,
        )?;

        let users =
            project::command::fetch_user(self.cache.clone(), self.auth0.clone(), cmd.clone())
                .await?;

        let records = users.into_iter().map(|v| v.into()).collect();
        let message = proto::FetchProjectUsersResponse { records };

        Ok(tonic::Response::new(message))
    }
    async fn fetch_project_user_invites(
        &self,
        request: tonic::Request<proto::FetchProjectUserInvitesRequest>,
    ) -> Result<tonic::Response<proto::FetchProjectUserInvitesResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = project::command::FetchUserInviteCmd::new(
            credential,
            req.page,
            req.page_size,
            req.project_id,
        )?;

        let invites = project::command::fetch_user_invite(self.cache.clone(), cmd.clone()).await?;

        let records = invites.into_iter().map(|v| v.into()).collect();
        let message = proto::FetchProjectUserInvitesResponse { records };

        Ok(tonic::Response::new(message))
    }

    async fn create_project_user_invite(
        &self,
        request: tonic::Request<proto::CreateProjectUserInviteRequest>,
    ) -> Result<tonic::Response<proto::CreateProjectUserInviteResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = project::command::CreateUserInviteCmd::try_new(
            credential,
            self.invite_ttl,
            req.project_id,
            req.email,
            req.role.parse()?,
        )?;

        project::command::create_user_invite(
            self.cache.clone(),
            self.email.clone(),
            self.event.clone(),
            cmd.clone(),
        )
        .await?;

        let message = proto::CreateProjectUserInviteResponse {};

        Ok(tonic::Response::new(message))
    }

    async fn accept_project_user_invite(
        &self,
        request: tonic::Request<proto::AcceptProjectUserInviteRequest>,
    ) -> Result<tonic::Response<proto::AcceptProjectUserInviteResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = project::command::AcceptUserInviteCmd::new(credential, req.code);

        project::command::accept_user_invite(
            self.cache.clone(),
            self.auth0.clone(),
            self.event.clone(),
            cmd.clone(),
        )
        .await?;

        let message = proto::AcceptProjectUserInviteResponse {};

        Ok(tonic::Response::new(message))
    }
    async fn resend_project_user_invite(
        &self,
        request: tonic::Request<proto::ResendProjectUserInviteRequest>,
    ) -> Result<tonic::Response<proto::ResendProjectUserInviteResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = project::command::ResendUserInviteCmd::new(credential, req.id);

        project::command::resend_user_invite(self.cache.clone(), self.email.clone(), cmd.clone())
            .await?;

        let message = proto::ResendProjectUserInviteResponse {};

        Ok(tonic::Response::new(message))
    }
    async fn delete_project_user_invite(
        &self,
        request: tonic::Request<proto::DeleteProjectUserInviteRequest>,
    ) -> Result<tonic::Response<proto::DeleteProjectUserInviteResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();

        let cmd = project::command::DeleteUserInviteCmd::new(credential, req.id);

        project::command::delete_user_invite(self.cache.clone(), self.event.clone(), cmd.clone())
            .await?;

        let message = proto::DeleteProjectUserInviteResponse {};

        Ok(tonic::Response::new(message))
    }

    async fn delete_project_user(
        &self,
        request: tonic::Request<proto::DeleteProjectUserRequest>,
    ) -> Result<tonic::Response<proto::DeleteProjectUserResponse>, tonic::Status> {
        let credential = match request.extensions().get::<Credential>() {
            Some(credential) => credential.clone(),
            None => return Err(Status::unauthenticated("invalid credential")),
        };

        let req = request.into_inner();
        let cmd = project::command::DeleteUserCmd::new(credential, req.project_id, req.id);
        project::command::delete_user(self.cache.clone(), self.event.clone(), cmd.clone()).await?;
        let message = proto::DeleteProjectUserResponse {};

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
            billing_provider: value.billing_provider,
            billing_provider_id: value.billing_provider_id,
            billing_subscription_id: value.billing_subscription_id,
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<ProjectUserInvite> for proto::ProjectUserInvite {
    fn from(value: ProjectUserInvite) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            email: value.email,
            role: value.role.to_string(),
            status: value.status.to_string(),
            expires_in: value.expires_in.to_rfc3339(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<ProjectUserAggregated> for proto::ProjectUser {
    fn from(value: ProjectUserAggregated) -> Self {
        Self {
            user_id: value.user_id,
            name: value.name,
            email: value.email,
            project_id: value.project_id,
            role: value.role.to_string(),
            created_at: value.created_at.to_rfc3339(),
        }
    }
}

impl From<ProjectSecret> for proto::ProjectSecret {
    fn from(value: ProjectSecret) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            name: value.name,
            created_at: value.created_at.to_rfc3339(),
        }
    }
}
