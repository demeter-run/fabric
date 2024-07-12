use dmtri::demeter::ops::v1alpha as proto;
use std::sync::Arc;
use tonic::{async_trait, Status};

use crate::domain::{
    events::EventBridge,
    management::{
        self,
        user::{AuthProvider, UserCache},
    },
};

pub struct UserServiceImpl {
    pub cache: Arc<dyn UserCache>,
    pub auth: Arc<dyn AuthProvider>,
    pub event: Arc<dyn EventBridge>,
}

//TODO: remove later
#[allow(dead_code)]
impl UserServiceImpl {
    pub fn new(
        cache: Arc<dyn UserCache>,
        auth: Arc<dyn AuthProvider>,
        event: Arc<dyn EventBridge>,
    ) -> Self {
        Self { cache, auth, event }
    }
}

#[async_trait]
impl proto::user_service_server::UserService for UserServiceImpl {
    async fn create_user(
        &self,
        request: tonic::Request<proto::CreateUserRequest>,
    ) -> Result<tonic::Response<proto::CreateUserResponse>, tonic::Status> {
        let req = request.into_inner();

        let result = management::user::create(
            self.cache.clone(),
            self.auth.clone(),
            self.event.clone(),
            req.token,
        )
        .await;

        if let Err(err) = result {
            return Err(Status::failed_precondition(err.to_string()));
        }
        let user = result.unwrap();

        let message = proto::CreateUserResponse {
            id: user.id,
            email: user.email,
        };
        Ok(tonic::Response::new(message))
    }
}
