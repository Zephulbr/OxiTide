use base64::Engine;
use sha2::{Digest, Sha256};

pub const PKCE_CLIENT_ID: &str = "6BDSRdpK9hqEBTgU";
pub const PKCE_REDIRECT_URI: &str = "https://tidal.com/android/login/auth";
pub const AUTHORIZE_URL: &str = "https://login.tidal.com/authorize";
pub const TOKEN_URL: &str = "https://auth.tidal.com/v1/oauth2/token";

#[derive(Debug, Clone)]
pub struct Pkce {
    pub code_verifier: String,
    pub code_challenge: String,
    pub client_unique_key: String,
}

impl Pkce {
    pub fn from_parts(code_verifier: String, client_unique_key: String) -> Self {
        let code_challenge = code_challenge(&code_verifier);
        Self { code_verifier, code_challenge, client_unique_key }
    }

    pub fn generate() -> Self {
        let verifier = random_verifier();
        let key = random_unique_key();
        Self::from_parts(verifier, key)
    }

    pub fn login_url(&self, client_id: &str) -> String {
        format!(
            "{base}?response_type=code&redirect_uri={redirect}&client_id={client}&lang=EN\
&appMode=android&client_unique_key={key}&code_challenge={challenge}\
&code_challenge_method=S256&restrict_signup=true",
            base = AUTHORIZE_URL,
            redirect = urlencode(PKCE_REDIRECT_URI),
            client = client_id,
            key = self.client_unique_key,
            challenge = self.code_challenge,
        )
    }
}

/// S256 code challenge: base64url(sha256(verifier)) without padding.
pub fn code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

fn random_verifier() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn random_unique_key() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    format!("{:016x}", u64::from_be_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s256_matches_rfc7636_appendix_b() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(code_challenge(verifier), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn login_url_contains_expected_params() {
        let pkce = Pkce::from_parts("verifier-abc".into(), "0011223344556677".into());
        let url = pkce.login_url(PKCE_CLIENT_ID);
        assert!(url.starts_with(AUTHORIZE_URL));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("appMode=android"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Ftidal.com%2Fandroid%2Flogin%2Fauth"));
        assert!(url.contains(&format!("code_challenge={}", pkce.code_challenge)));
    }

    #[test]
    fn generate_is_nonthrowing() {
        let p = Pkce::generate();
        assert!(p.code_verifier.len() >= 43);
        assert!(!p.code_challenge.is_empty());
    }
}
