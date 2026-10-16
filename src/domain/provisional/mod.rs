//! Projects an anonymous agent registers before any person owns them.
//!
//! The protocol is published in the `devex` contract "registration and claim": a provisional
//! project is limited to Free testnet ports until a person claims it, and expires unclaimed after
//! `ttl_days`. The error details below are the contract's codes; each one is the whole message of
//! the gRPC status it travels in, so the Console can map it without parsing prose.

use base64::{prelude::BASE64_URL_SAFE_NO_PAD, Engine};
use rand::{rngs::OsRng, Rng, RngCore};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use super::{error::Error, resource::command::Spec, Result};

pub mod command;

pub const REGISTRATION_DISABLED: &str = "registration_disabled";
pub const IP_CAP: &str = "ip_cap";
pub const GLOBAL_CAP: &str = "global_cap";
pub const PRE_CLAIM_LIMIT: &str = "pre_claim_limit";
pub const PORT_CAP: &str = "port_cap";
pub const EXPIRED: &str = "expired";

/// The only networks a provisional project's ports may use.
pub const PRE_CLAIM_NETWORKS: [&str; 2] = ["cardano-preprod", "cardano-preview"];
/// The only throughput tier a provisional project's ports may use: Free.
pub const PRE_CLAIM_TIER: &str = "0";

const CLAIM_TOKEN_BYTES: usize = 32;
const USER_CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const USER_CODE_LEN: usize = 8;

/// The `[provisional]` table of the rpc config. Every field has a default that is safe to deploy:
/// registration is off and no gateway token is accepted.
#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct ProvisionalConfig {
    /// The kill switch: while false, `Register` refuses every call. Nothing else depends on it.
    pub enabled: bool,
    /// The token the Console presents in `dmtr-gateway-token`. Empty refuses every call.
    pub gateway_token: String,
    /// Registrations allowed from one client IP in the trailing 24 hours.
    pub ip_cap: u32,
    /// Provisional projects allowed to be live at once.
    pub global_cap: u32,
    /// Live ports allowed on one provisional project.
    pub port_cap: u32,
    pub ttl_days: u32,
    /// How often the rpc expires provisional projects, whether or not registration is on.
    pub sweep_interval_secs: u64,
}
impl std::fmt::Debug for ProvisionalConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProvisionalConfig")
            .field("enabled", &self.enabled)
            .field("gateway_token", &"<redacted>")
            .field("ip_cap", &self.ip_cap)
            .field("global_cap", &self.global_cap)
            .field("port_cap", &self.port_cap)
            .field("ttl_days", &self.ttl_days)
            .field("sweep_interval_secs", &self.sweep_interval_secs)
            .finish()
    }
}
impl Default for ProvisionalConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            gateway_token: String::new(),
            ip_cap: 3,
            global_cap: 200,
            port_cap: 3,
            ttl_days: 7,
            sweep_interval_secs: 600,
        }
    }
}

/// Whether `presented` is the configured gateway token, compared in constant time. An empty
/// configured token matches nothing, so a deployment that never set one stays closed.
pub fn gateway_token_matches(configured: &str, presented: Option<&str>) -> bool {
    let Some(presented) = presented else {
        return false;
    };
    if configured.is_empty() {
        return false;
    }

    // Comparing digests keeps the comparison constant-time even when the lengths differ.
    sha256(configured.as_bytes())
        .ct_eq(&sha256(presented.as_bytes()))
        .into()
}

/// A claim token: 32 random bytes, base64url without padding, as it appears in the claim URL.
pub fn generate_claim_token() -> String {
    let mut bytes = [0u8; CLAIM_TOKEN_BYTES];
    OsRng.fill_bytes(&mut bytes);
    BASE64_URL_SAFE_NO_PAD.encode(bytes)
}

/// The stored form of a claim token: the lowercase hex SHA-256 of the token as the URL carries it.
pub fn hash_claim_token(token: &str) -> String {
    hex(&sha256(token.as_bytes()))
}

/// A user code in its stored form: 8 characters from an alphabet without look-alikes, no hyphen.
pub fn generate_user_code() -> String {
    (0..USER_CODE_LEN)
        .map(|_| USER_CODE_ALPHABET[OsRng.gen_range(0..USER_CODE_ALPHABET.len())] as char)
        .collect()
}

/// A user code as a person reads it: `XXXX-XXXX`.
pub fn display_user_code(code: &str) -> String {
    let (head, tail) = code.split_at(code.len() / 2);
    format!("{head}-{tail}")
}

/// A user code as typed, in its stored form: case-insensitive, hyphen optional. `None` when it
/// cannot be a user code at all, so the caller answers as for an unknown one without a lookup.
pub fn normalize_user_code(input: &str) -> Option<String> {
    let code: String = input
        .trim()
        .chars()
        .filter(|c| *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();

    let valid =
        code.len() == USER_CODE_LEN && code.bytes().all(|c| USER_CODE_ALPHABET.contains(&c));
    valid.then_some(code)
}

/// The salted hash the IP cap counts by, so fabric never stores the client IP itself.
pub fn hash_source_ip(client_ip: &str, secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(client_ip.as_bytes());
    hex(&hasher.finalize())
}

/// Refuses a port spec a provisional project may not have: a network other than preprod or
/// preview, or a throughput tier other than Free.
pub fn assert_pre_claim_spec(spec: &Spec) -> Result<()> {
    let network = spec.get("network").and_then(|v| v.as_str());
    let tier = spec.get("throughputTier").and_then(|v| v.as_str());

    let allowed =
        network.is_some_and(|n| PRE_CLAIM_NETWORKS.contains(&n)) && tier == Some(PRE_CLAIM_TIER);
    if !allowed {
        return Err(Error::Unauthorized(PRE_CLAIM_LIMIT.into()));
    }

    Ok(())
}

fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(json: &str) -> Spec {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn it_should_default_to_registration_off_with_the_contract_limits() {
        let config: ProvisionalConfig = serde_json::from_str("{}").unwrap();

        assert!(!config.enabled);
        assert!(config.gateway_token.is_empty());
        assert_eq!(config.ip_cap, 3);
        assert_eq!(config.global_cap, 200);
        assert_eq!(config.port_cap, 3);
        assert_eq!(config.ttl_days, 7);
        assert_eq!(config.sweep_interval_secs, 600);

        let config: ProvisionalConfig =
            serde_json::from_str(r#"{"enabled":true,"gateway_token":"s3cr3t-gateway"}"#).unwrap();
        assert!(config.enabled);
        assert_eq!(config.ip_cap, 3);
        assert!(!format!("{config:?}").contains("s3cr3t-gateway"));
    }

    #[test]
    fn it_should_match_only_the_configured_gateway_token() {
        assert!(gateway_token_matches("gateway", Some("gateway")));
        assert!(!gateway_token_matches("gateway", Some("other")));
        assert!(!gateway_token_matches("gateway", Some("")));
        assert!(!gateway_token_matches("gateway", None));
        assert!(!gateway_token_matches("", Some("")));
        assert!(!gateway_token_matches("", None));
    }

    #[test]
    fn it_should_generate_a_claim_token_of_32_bytes() {
        let token = generate_claim_token();
        let bytes = BASE64_URL_SAFE_NO_PAD.decode(&token).unwrap();

        assert_eq!(bytes.len(), CLAIM_TOKEN_BYTES);
        assert_ne!(token, generate_claim_token());
        assert_eq!(hash_claim_token(&token).len(), 64);
    }

    #[test]
    fn it_should_generate_user_codes_from_the_alphabet() {
        for _ in 0..100 {
            let code = generate_user_code();
            assert_eq!(code.len(), USER_CODE_LEN);
            assert!(code.bytes().all(|c| USER_CODE_ALPHABET.contains(&c)));
        }
        assert_eq!(display_user_code("ABCD2345"), "ABCD-2345");
    }

    #[test]
    fn it_should_normalize_a_typed_user_code() {
        for typed in [
            "ABCD-2345",
            "abcd-2345",
            "ABCD2345",
            "abcd2345",
            " AbCd-2345 ",
        ] {
            assert_eq!(
                normalize_user_code(typed).as_deref(),
                Some("ABCD2345"),
                "{typed}"
            );
        }
        for typed in [
            "",
            "ABCD-234",
            "ABCD-23456",
            "ABCD-2341",
            "ABCO-2345",
            "ABCD_2345",
        ] {
            assert_eq!(normalize_user_code(typed), None, "{typed}");
        }
    }

    #[test]
    fn it_should_salt_the_source_ip_hash() {
        let hash = hash_source_ip("203.0.113.7", "secret");

        assert_eq!(hash, hash_source_ip("203.0.113.7", "secret"));
        assert_ne!(hash, hash_source_ip("203.0.113.8", "secret"));
        assert_ne!(hash, hash_source_ip("203.0.113.7", "other secret"));
        assert!(!hash.contains("203.0.113.7"));
    }

    #[test]
    fn it_should_accept_free_testnet_specs() {
        for network in PRE_CLAIM_NETWORKS {
            let spec = spec(&format!(
                r#"{{"network":"{network}","throughputTier":"0"}}"#
            ));
            assert!(assert_pre_claim_spec(&spec).is_ok());
        }
    }

    #[test]
    fn it_should_refuse_mainnet_and_paid_tiers() {
        for json in [
            r#"{"network":"cardano-mainnet","throughputTier":"0"}"#,
            r#"{"network":"cardano-preprod","throughputTier":"1"}"#,
            r#"{"network":"cardano-preprod","throughputTier":0}"#,
            r#"{"network":"cardano-preprod"}"#,
            r#"{"throughputTier":"0"}"#,
        ] {
            let result = assert_pre_claim_spec(&spec(json));
            assert!(
                matches!(&result, Err(Error::Unauthorized(code)) if code == PRE_CLAIM_LIMIT),
                "{json} was not refused"
            );
        }
    }
}
