use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    CommandMalformed(String),
    #[error("{0}")]
    SecretExceeded(String),
    #[error("{0}")]
    Unexpected(String),
}

impl From<argon2::password_hash::Error> for Error {
    fn from(value: argon2::password_hash::Error) -> Self {
        Self::Unexpected(value.to_string())
    }
}
impl From<argon2::Error> for Error {
    fn from(value: argon2::Error) -> Self {
        Self::Unexpected(value.to_string())
    }
}
impl From<bech32::EncodeError> for Error {
    fn from(value: bech32::EncodeError) -> Self {
        Self::Unexpected(value.to_string())
    }
}
impl From<bech32::primitives::hrp::Error> for Error {
    fn from(value: bech32::primitives::hrp::Error) -> Self {
        Self::Unexpected(value.to_string())
    }
}
impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Unexpected(value.to_string())
    }
}
impl From<sqlx::Error> for Error {
    fn from(value: sqlx::Error) -> Self {
        Self::Unexpected(value.to_string())
    }
}
impl From<kube::Error> for Error {
    fn from(value: kube::Error) -> Self {
        Self::Unexpected(value.to_string())
    }
}
