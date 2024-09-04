use aws_config::Region;
use aws_sdk_sesv2::{
    config::{Credentials, SharedCredentialsProvider},
    types::{Destination, EmailContent, Template},
    Client,
};
use chrono::{DateTime, Utc};
use serde_json::json;

use crate::domain::{error::Error, project::ProjectEmailDriven, Result};

pub struct SESDrivenImpl {
    client: Client,
    verified_email: String,
}
impl SESDrivenImpl {
    pub fn new(
        access_key_id: &str,
        secret_access_key: &str,
        region: &str,
        verified_email: &str,
    ) -> Self {
        let credentials = Credentials::new(
            access_key_id,
            secret_access_key,
            None,
            None,
            "StaticCredentials",
        );
        let region = Region::new(region.to_string());
        let config = aws_config::SdkConfig::builder()
            .region(region)
            .credentials_provider(SharedCredentialsProvider::new(credentials))
            .build();

        let client = Client::new(&config);
        let verified_email = verified_email.to_string();

        Self {
            client,
            verified_email,
        }
    }
}

#[async_trait::async_trait]
impl ProjectEmailDriven for SESDrivenImpl {
    async fn send_invite(
        &self,
        project_name: &str,
        email: &str,
        code: &str,
        expires_in: &DateTime<Utc>,
    ) -> Result<()> {
        let destination = Destination::builder().to_addresses(email).build();
        let template = Template::builder()
            .template_name("invite")
            .template_data(json!({ "project_name": project_name, "code": code, "expires_in": expires_in.to_rfc2822() }).to_string())
            .build();
        let email_content = EmailContent::builder().template(template).build();

        self.client
            .send_email()
            .from_email_address(&self.verified_email)
            .destination(destination)
            .content(email_content)
            .send()
            .await
            .map_err(|err| Error::Unexpected(err.to_string()))?;

        Ok(())
    }
}
