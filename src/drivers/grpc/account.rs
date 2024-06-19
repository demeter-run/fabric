use dmtri::demeter::ops::v1alpha as proto;
use std::sync::Arc;
use tonic::{async_trait, Status};

use crate::domain::{
    events::EventBridge,
    management::{
        self,
        account::{Account, AccountCache},
    },
};

pub struct AccountServiceImpl {
    pub cache: Arc<dyn AccountCache>,
    pub event: Arc<dyn EventBridge>,
}

//TODO: remove later
#[allow(dead_code)]
impl AccountServiceImpl {
    pub fn new(cache: Arc<dyn AccountCache>, event: Arc<dyn EventBridge>) -> Self {
        Self { cache, event }
    }
}

#[async_trait]
impl proto::account_service_server::AccountService for AccountServiceImpl {
    async fn create_account(
        &self,
        request: tonic::Request<proto::CreateAccountRequest>,
    ) -> Result<tonic::Response<proto::CreateAccountResponse>, tonic::Status> {
        let req = request.into_inner();

        let account = Account::new(req.name);
        let result =
            management::account::create(self.cache.clone(), self.event.clone(), account.clone())
                .await;

        if let Err(err) = result {
            return Err(Status::failed_precondition(err.to_string()));
        }

        let message = proto::CreateAccountResponse { name: account.name };
        Ok(tonic::Response::new(message))
    }
}
