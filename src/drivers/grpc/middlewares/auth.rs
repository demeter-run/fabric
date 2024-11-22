use std::sync::Arc;

use crate::{
    domain::{
        auth::{Auth0Driven, Credential},
        project::{self, cache::ProjectDrivenCache},
    },
    driven::prometheus::metrics::MetricsDriven,
    drivers::grpc::handle_error_metric,
};

#[derive(Clone)]
pub struct AuthenticatorImpl {
    auth0: Arc<dyn Auth0Driven>,
    cache: Arc<dyn ProjectDrivenCache>,
    metrics: Arc<MetricsDriven>,
}
impl AuthenticatorImpl {
    pub fn new(
        auth0: Arc<dyn Auth0Driven>,
        cache: Arc<dyn ProjectDrivenCache>,
        metrics: Arc<MetricsDriven>,
    ) -> Self {
        Self {
            auth0,
            cache,
            metrics,
        }
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
                Err(err) => {
                    handle_error_metric(self.metrics.clone(), "auth", &err);
                    Err(tonic::Status::permission_denied(
                        "invalid authorization token",
                    ))
                }
            };
        }

        if let Some(token) = extract_metadata_string(&request, "dmtr-api-key") {
            let project = extract_metadata_string(&request, "project-id")
                .or_else(|| extract_metadata_string(&request, "project-namespace"))
                .ok_or_else(|| {
                    tonic::Status::permission_denied("project-id or project-namespace is required")
                })?;

            return tokio::task::block_in_place(|| {
                return tokio::runtime::Runtime::new().unwrap().block_on(async {
                    let cmd = project::command::VerifySecretCmd {
                        key: token,
                        project,
                    };

                    let secret = project::command::verify_secret(self.cache.clone(), cmd)
                        .await
                        .inspect_err(|err| {
                            handle_error_metric(self.metrics.clone(), "auth", err)
                        })?;

                    let credential = Credential::ApiKey(secret.project_id);
                    request.extensions_mut().insert(credential);

                    Ok(request)
                });
            });
        }

        Err(tonic::Status::unauthenticated("authentication is required"))
    }
}
