use std::sync::Arc;

use regex::Regex;

use crate::domain::users::{AuthProvider, Credential};

#[derive(Clone)]
pub struct AuthenticatorImpl {
    auth_provider: Arc<dyn AuthProvider>,
    token_regex: Regex,
}
impl AuthenticatorImpl {
    pub fn new(auth_provider: Arc<dyn AuthProvider>) -> Self {
        let token_regex = Regex::new(r"^(?:Bearer\s+)?(.+)$").unwrap();

        Self {
            auth_provider,
            token_regex,
        }
    }
}

impl tonic::service::Interceptor for AuthenticatorImpl {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        let Some(token) = request.metadata().get("Authorization") else {
            return Err(tonic::Status::unauthenticated("authentication is required"));
        };

        let Ok(token) = token.to_str() else {
            return Err(tonic::Status::unauthenticated("authentication malformed"));
        };

        let token = self
            .token_regex
            .captures(token)
            .unwrap()
            .get(1)
            .unwrap()
            .as_str();

        let Ok(user_id) = self.auth_provider.verify(token) else {
            return Err(tonic::Status::unauthenticated("invalid authentication"));
        };

        let credential = Credential { id: user_id };

        request.extensions_mut().insert(credential);

        Ok(request)
    }
}
