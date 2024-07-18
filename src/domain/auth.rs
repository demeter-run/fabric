pub type UserId = String;
pub type ApiKey = Vec<u8>;

#[derive(Debug, Clone)]
pub enum Credential {
    Auth0(UserId),
    ApiKey(ApiKey),
}
