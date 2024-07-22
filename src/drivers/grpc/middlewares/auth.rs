use std::sync::Arc;

use crate::{
    domain::{
        auth::Credential,
        project::{self, ProjectDrivenCache},
    },
    driven::auth::Auth0Provider,
};

#[derive(Clone)]
pub struct AuthenticatorImpl {
    auth0: Arc<Auth0Provider>,
    cache: Arc<dyn ProjectDrivenCache>,
}
impl AuthenticatorImpl {
    pub fn new(auth0: Arc<Auth0Provider>, cache: Arc<dyn ProjectDrivenCache>) -> Self {
        Self { auth0, cache }
    }
}

fn extract_metadata_string(request: &tonic::Request<()>, key: &str) -> Option<String> {
    let metadata = request.metadata().get(key)?;
    let Ok(key) = metadata.to_str() else {
        return None;
    };

    Some(key.into())
}

impl tonic::service::Interceptor for AuthenticatorImpl {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        if let Some(token) = extract_metadata_string(&request, "Authorization") {
            let token = token.replace("Bearer ", "");
            return match self.auth0.verify(&token) {
                Ok(user_id) => {
                    let credential = Credential::Auth0(user_id);
                    request.extensions_mut().insert(credential);
                    Ok(request)
                }
                Err(_) => Err(tonic::Status::unauthenticated("invalid authentication")),
            };
        }

        if let Some(token) = extract_metadata_string(&request, "dmtr-api-key") {
            let Some(project_id) = extract_metadata_string(&request, "project-id") else {
                return Err(tonic::Status::permission_denied("project-id is required"));
            };
            return tokio::task::block_in_place(|| {
                return tokio::runtime::Runtime::new().unwrap().block_on(async {
                    match project::verify_secret(self.cache.clone(), &project_id, &token).await {
                        Ok(secret) => {
                            let credential = Credential::ApiKey(secret.project_id);
                            request.extensions_mut().insert(credential);
                            Ok(request)
                        }
                        Err(_) => Err(tonic::Status::permission_denied("invalid project secret")),
                    }
                });
            });
        }

        Err(tonic::Status::unauthenticated("authentication is required"))
    }
}
