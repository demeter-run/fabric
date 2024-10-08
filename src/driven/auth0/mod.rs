use anyhow::Result as AnyhowResult;
use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet};
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};
use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::domain::auth::Auth0Profile;
use crate::domain::error::Error;
use crate::domain::{auth::Auth0Driven, Result};

pub struct Auth0DrivenImpl {
    client: Client,
    url: String,
    client_id: String,
    client_secret: String,
    audience: String,
    jwks: JwkSet,
}
impl Auth0DrivenImpl {
    pub async fn try_new(
        url: &str,
        client_id: &str,
        client_secret: &str,
        audience: &str,
    ) -> AnyhowResult<Self> {
        let client = Client::new();
        let url = url.to_string();
        let client_id = client_id.to_string();
        let client_secret = client_secret.to_string();
        let audience = audience.to_string();

        let jwks_request = client
            .get(format!("{}/.well-known/jwks.json", url))
            .build()?;
        let jwks_response = client.execute(jwks_request).await?;
        let jwks = jwks_response.json().await?;

        Ok(Self {
            client,
            url,
            client_id,
            client_secret,
            audience,
            jwks,
        })
    }

    async fn auth0_api_token(&self) -> Result<String> {
        let request_payload = RequestAccessToken {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            audience: self.audience.clone(),
            grant_type: "client_credentials".into(),
        };

        // TODO: consider token expiration
        let access_token_response = self
            .client
            .post(format!("{}/oauth/token", &self.url))
            .json(&request_payload)
            .send()
            .await?;

        let access_token_status = access_token_response.status();
        if access_token_status.is_client_error() || access_token_status.is_server_error() {
            error!(
                status = access_token_status.to_string(),
                "Auth0 request error to get access token"
            );
            return Err(Error::Unexpected(format!(
                "Auth0 request error to get access token. Status: {}",
                access_token_status
            )));
        }
        let access_token = access_token_response
            .json::<ResponseAccessToken>()
            .await?
            .access_token;

        Ok(access_token)
    }
}

#[async_trait::async_trait]
impl Auth0Driven for Auth0DrivenImpl {
    fn verify(&self, token: &str) -> Result<String> {
        let header = decode_header(token).map_err(|err| Error::Unexpected(err.to_string()))?;

        let Some(kid) = header.kid else {
            return Err(Error::Unexpected(
                "token doesnt have a `kid` header field".into(),
            ));
        };
        let Some(jwk) = self.jwks.find(&kid) else {
            return Err(Error::Unexpected(
                "no matching jwk found for the given kid".into(),
            ));
        };

        let decoding_key = match &jwk.algorithm {
            AlgorithmParameters::RSA(rsa) => DecodingKey::from_rsa_components(&rsa.n, &rsa.e)
                .map_err(|err| Error::Unexpected(err.to_string())),
            _ => Err(Error::Unexpected("algorithm should be a RSA".into())),
        }?;

        let validation = {
            let mut validation = Validation::new(header.alg);
            validation.set_audience(&["demeter-api"]);
            validation.validate_exp = true;
            validation
        };

        let decoded_token = decode::<Claims>(token, &decoding_key, &validation)
            .map_err(|err| Error::Unexpected(err.to_string()))?;

        Ok(decoded_token.claims.sub)
    }

    async fn find_info(&self, user_id: &str) -> Result<Auth0Profile> {
        let access_token = self.auth0_api_token().await?;

        let profile_response = self
            .client
            .get(format!("{}/api/v2/users/{user_id}", &self.url))
            .header(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {access_token}")).unwrap(),
            )
            .send()
            .await?;

        let profile_status = profile_response.status();
        if profile_status.is_client_error() || profile_status.is_server_error() {
            error!(
                status = profile_status.to_string(),
                "Auth0 request error to get user info"
            );
            return Err(Error::Unexpected(format!(
                "Auth0 request error to get user info. Status: {}",
                profile_status
            )));
        }
        let profile = profile_response.json::<Auth0Profile>().await?;

        Ok(profile)
    }

    async fn find_info_by_ids(&self, ids: &[String]) -> Result<Vec<Auth0Profile>> {
        let access_token = self.auth0_api_token().await?;

        let profile_response = self
            .client
            .get(format!("{}/api/v2/users", &self.url))
            .query(&[("q", format!("id {}", ids.join(" ")))])
            .header(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {access_token}")).unwrap(),
            )
            .send()
            .await?;

        let profile_status = profile_response.status();
        if profile_status.is_client_error() || profile_status.is_server_error() {
            error!(
                status = profile_status.to_string(),
                "Auth0 request error to get user info"
            );
            return Err(Error::Unexpected(format!(
                "Auth0 request error to get user info. Status: {}",
                profile_status
            )));
        }

        let profiles = profile_response.json::<Vec<Auth0Profile>>().await?;

        Ok(profiles)
    }
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
}

#[derive(Serialize)]
struct RequestAccessToken {
    client_id: String,
    client_secret: String,
    audience: String,
    grant_type: String,
}
#[derive(Deserialize)]
struct ResponseAccessToken {
    access_token: String,
}
