pub type UserId = String;
pub type SecretId = String;

#[derive(Debug, Clone)]
pub enum Credential {
    Auth0(UserId),
    ApiKey(SecretId),
}
