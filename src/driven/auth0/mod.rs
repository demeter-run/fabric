use anyhow::Result as AnyhowResult;
use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet};
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};
use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::Client;
use serde::Deserialize;
use tracing::error;

use crate::domain::error::Error;
use crate::domain::{auth::Auth0Driven, Result};

pub struct Auth0DrivenImpl {
    client: Client,
    url: String,
    jwks: JwkSet,
}
impl Auth0DrivenImpl {
    pub async fn try_new(url: &str) -> AnyhowResult<Self> {
        let client = Client::new();
        let url = url.to_string();

        let jwks_request = client
            .get(format!("{}/.well-known/jwks.json", url))
            .build()?;

        let jwks_response = client.execute(jwks_request).await?;
        let jwks = jwks_response.json().await?;

        Ok(Self { client, url, jwks })
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

    async fn find_info(&self, token: &str) -> Result<(String, String)> {
        let response = self
            .client
            .get(format!("{}/userinfo", &self.url))
            .header(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
            )
            .send()
            .await?;

        let status = response.status();
        if status.is_client_error() || status.is_server_error() {
            error!(
                status = status.to_string(),
                "request status code fail to get auth0 user info"
            );
            return Err(Error::Unexpected(format!(
                "Auth0 request error to get user info. Status: {}",
                status
            )));
        }

        let profile: Profile = response.json().await?;

        Ok((profile.name, profile.email))
    }
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
}

#[derive(Deserialize)]
struct Profile {
    name: String,
    email: String,
}
