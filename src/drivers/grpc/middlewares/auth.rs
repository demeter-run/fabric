use std::sync::Arc;

use tracing::warn;

use crate::{domain::auth::Credential, driven::auth::Auth0Provider};

#[derive(Clone)]
pub struct AuthenticatorImpl {
    auth0: Arc<Auth0Provider>,
}
impl AuthenticatorImpl {
    pub fn new(auth0: Arc<Auth0Provider>) -> Self {
        Self { auth0 }
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
            let Ok(user_id) = self.auth0.verify(&token) else {
                return Err(tonic::Status::unauthenticated("invalid authentication"));
            };
            let credential = Credential::Auth0(user_id);
            request.extensions_mut().insert(credential);

            return Ok(request);
        }

        if let Some(token) = extract_metadata_string(&request, "dmtr-api-key") {
            let Some(project_id) = extract_metadata_string(&request, "project-id") else {
                return Err(tonic::Status::permission_denied("project-id is required"));
            };

            let (hrp, key) = bech32::decode(&token).map_err(|error| {
                warn!(?error, "invalid bech32");
                tonic::Status::permission_denied("invalid apikey")
            })?;

            if !hrp.to_string().eq("dmtr_apikey") {
                warn!(hrp = hrp.to_string(), "invalid bech32 hrp");
                return Err(tonic::Status::permission_denied("invalid apikey"));
            }

            let credential = Credential::ApiKey(key, project_id);
            request.extensions_mut().insert(credential);
            return Ok(request);
        }

        Err(tonic::Status::unauthenticated("authentication is required"))
    }
}
