use anyhow::{bail, Result};
use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet};
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};
use serde::Deserialize;

use crate::domain::management::user;

pub struct Auth0Provider {
    client: reqwest::Client,
    url: String,
}
impl Auth0Provider {
    pub fn new(url: &str) -> Self {
        let client = reqwest::Client::new();
        Self {
            client,
            url: url.into(),
        }
    }
}

#[async_trait::async_trait]
impl user::AuthProvider for Auth0Provider {
    async fn verify(&self, token: &str) -> Result<String> {
        let jwks_request = self
            .client
            .get(format!("{}/.well-known/jwks.json", self.url))
            .build()?;

        let jwks_response = self.client.execute(jwks_request).await?;
        let jwks: JwkSet = jwks_response.json().await?;

        let header = decode_header(token)?;

        let Some(kid) = header.kid else {
            bail!("token doesn't have a `kid` header field");
        };
        let Some(jwk) = jwks.find(&kid) else {
            bail!("no matching jwk found for the given kid");
        };

        let decoding_key = match &jwk.algorithm {
            AlgorithmParameters::RSA(rsa) => DecodingKey::from_rsa_components(&rsa.n, &rsa.e)?,
            _ => bail!("algorithm should be a RSA"),
        };

        let validation = {
            let mut validation = Validation::new(header.alg);
            validation.set_audience(&["demeter-api"]);
            validation.validate_exp = true;
            validation
        };

        let decoded_token = decode::<Claims>(token, &decoding_key, &validation)?;

        Ok(decoded_token.claims.sub)
    }
    async fn get_profile(&self, token: &str) -> Result<String> {
        let profile_request = self
            .client
            .get(format!("{}/userinfo", self.url))
            .header("Authorization", format!("Bearer {token}"))
            .build()?;

        let profile_response = self
            .client
            .execute(profile_request)
            .await?
            .error_for_status()?;

        let profile = profile_response.json::<UserInfo>().await?;

        Ok(profile.email)
    }
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
}
#[derive(Deserialize)]
struct UserInfo {
    email: String,
}
