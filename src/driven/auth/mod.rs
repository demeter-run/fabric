use anyhow::{bail, Result};
use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet};
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};
use serde::Deserialize;

use crate::domain::users::AuthProvider;

pub struct AuthProviderImpl {
    client: reqwest::Client,
    url: String,
}
impl AuthProviderImpl {
    pub fn new(url: &str) -> Self {
        let client = reqwest::Client::new();
        Self {
            client,
            url: url.into(),
        }
    }
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
}

#[async_trait::async_trait]
impl AuthProvider for AuthProviderImpl {
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
}
