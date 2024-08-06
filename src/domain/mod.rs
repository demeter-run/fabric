use error::Error;

pub mod auth;
pub mod error;
pub mod event;
pub mod project;
pub mod resource;
pub mod utils;

pub const PAGE_SIZE_DEFAULT: u32 = 12;
pub const PAGE_SIZE_MAX: u32 = 120;
pub const MAX_SECRET: usize = 2;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    pub const KEY: &str = "dmtr_apikey1g9gyswtcf3zxwd26v4x5jj3jw5wx3sn2";
    pub const PHC: &str = "$argon2id$v=19$m=19456,t=2,p=1$xVIt6Wr/bm1FewVhTr6zgA$nTO6EgGeOYZe7thACrHmFUWND40U4GEQCXKyvqzvRvs";
    pub const SECRET: &str = "fabric@txpipe";
    pub const INVALID_KEY: &str = "dmtr_apikey1xe6xzcjxv9nhycnz2ffnq6m02y7nat9e";
    pub const INVALID_HRP_KEY: &str = "dmtr_test18pp5vkjzfuuyzwpeg9gk2a2zvsylc5wg";
}
