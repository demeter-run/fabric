use anyhow::{bail, Result};
use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet};
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};
use serde::Deserialize;

use crate::domain::users::AuthProvider;

pub struct Auth0Provider {
    jwks: JwkSet,
}
impl Auth0Provider {
    pub async fn try_new(url: &str) -> Result<Self> {
        let client = reqwest::Client::new();
        let jwks_request = client
            .get(format!("{}/.well-known/jwks.json", url))
            .build()?;

        let jwks_response = client.execute(jwks_request).await?;
        let jwks = jwks_response.json().await?;

        let auth_provider = Self { jwks };

        Ok(auth_provider)
    }
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
}

#[async_trait::async_trait]
impl AuthProvider for Auth0Provider {
    fn verify(&self, token: &str) -> Result<String> {
        let header = decode_header(token)?;

        let Some(kid) = header.kid else {
            bail!("token doesnt have a `kid` header field");
        };
        let Some(jwk) = self.jwks.find(&kid) else {
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
