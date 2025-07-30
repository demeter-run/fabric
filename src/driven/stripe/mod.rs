use std::collections::HashMap;

use reqwest::Client;
use serde::Deserialize;
use tracing::error;

use crate::domain::{error::Error, project::StripeDriven, Result};

pub struct StripeDrivenImpl {
    client: Client,
    url: String,
    api_key: String,
}
impl StripeDrivenImpl {
    pub fn new(url: &str, api_key: &str) -> Self {
        let client = Client::new();
        let url = url.to_string();
        let api_key = api_key.to_string();

        Self {
            client,
            url,
            api_key,
        }
    }
}

#[async_trait::async_trait]
impl StripeDriven for StripeDrivenImpl {
    async fn create_customer(&self, name: &str, email: &str) -> Result<String> {
        let mut params = HashMap::new();
        params.insert("name", name);
        params.insert("email", email);

        let response = self
            .client
            .post(format!("{}/customers", &self.url))
            .basic_auth(&self.api_key, Some(""))
            .form(&params)
            .send()
            .await?;

        let status = response.status();
        if status.is_client_error() || status.is_server_error() {
            error!(
                status = status.to_string(),
                "request status code fail to create stripe customer"
            );
            return Err(Error::Unexpected(format!(
                "stripe create customer request error. Status: {status}"
            )));
        }

        let customer: StripeCustomer = response.json().await?;

        Ok(customer.id)
    }
}

#[derive(Deserialize)]
struct StripeCustomer {
    id: String,
}
