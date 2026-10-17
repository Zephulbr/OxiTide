use base64::Engine;
use sha2::{Digest, Sha256};

pub const PKCE_CLIENT_ID: &str = "6BDSRdpK9hqEBTgU";
/// Redirect captured by the OS and handed to OxiTide (primary flow).
pub const PKCE_REDIRECT_DEEP_LINK: &str = "tidal://login/auth";
/// Redirect that leaves a pasteable URL in the browser (fallback flow).
pub const PKCE_REDIRECT_PASTE: &str = "https://tidal.com/android/login/auth";
pub const AUTHORIZE_URL: &str = "https://login.tidal.com/authorize";
pub const TOKEN_URL: &str = "https://auth.tidal.com/v1/oauth2/token";

#[derive(Clone)]
pub struct Pkce {
    pub code_verifier: String,
    pub code_challenge: String,
    pub client_unique_key: String,
}

impl std::fmt::Debug for Pkce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pkce")
            .field("code_verifier", &"<redacted>")
            .field("code_challenge", &self.code_challenge)
            .field("client_unique_key", &"<redacted>")
            .finish()
    }
}

impl Pkce {
    pub fn from_parts(code_verifier: String, client_unique_key: String) -> Self {
        let code_challenge = code_challenge(&code_verifier);
        Self {
            code_verifier,
            code_challenge,
            client_unique_key,
        }
    }

    pub fn generate() -> Self {
        let verifier = random_verifier();
        let key = random_unique_key();
        Self::from_parts(verifier, key)
    }

    pub fn login_url(&self, client_id: &str, state: &str, redirect_uri: &str) -> String {
        format!(
            "{base}?response_type=code&redirect_uri={redirect}&client_id={client}&lang=EN\
&appMode=android&client_unique_key={key}&code_challenge={challenge}\
&code_challenge_method=S256&restrict_signup=true&state={state}",
            base = AUTHORIZE_URL,
            redirect = urlencode(redirect_uri),
            client = client_id,
            key = self.client_unique_key,
            challenge = self.code_challenge,
            state = urlencode(state),
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

/// `N` bytes from the OS random source. Like `rand`'s `OsRng`, which this
/// replaced, an unavailable source is fatal: PKCE must not run on weak bytes.
fn os_random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).expect("OS random source unavailable");
    bytes
}

fn random_verifier() -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(os_random::<32>())
}

fn random_unique_key() -> String {
    format!("{:016x}", u64::from_be_bytes(os_random::<8>()))
}

/// A URL-safe anti-forgery `state` value (16 random bytes, hex).
pub fn random_state() -> String {
    os_random::<16>().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s256_matches_rfc7636_appendix_b() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            code_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn login_url_contains_expected_params() {
        let pkce = Pkce::from_parts("verifier-abc".into(), "0011223344556677".into());
        let url = pkce.login_url(PKCE_CLIENT_ID, "state-123", PKCE_REDIRECT_DEEP_LINK);
        assert!(url.starts_with(AUTHORIZE_URL));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("appMode=android"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("redirect_uri=tidal%3A%2F%2Flogin%2Fauth"));
        assert!(url.contains("state=state-123"));
        assert!(url.contains(&format!("code_challenge={}", pkce.code_challenge)));
    }

    #[test]
    fn login_url_paste_mode_uses_the_https_redirect() {
        let pkce = Pkce::from_parts("v".into(), "k".into());
        let url = pkce.login_url(PKCE_CLIENT_ID, "s", PKCE_REDIRECT_PASTE);
        assert!(url.contains("redirect_uri=https%3A%2F%2Ftidal.com%2Fandroid%2Flogin%2Fauth"));
    }

    #[test]
    fn random_state_is_hex_and_unique() {
        let a = random_state();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, random_state());
    }

    #[test]
    fn generate_is_nonthrowing() {
        let p = Pkce::generate();
        assert!(p.code_verifier.len() >= 43);
        assert!(!p.code_challenge.is_empty());
    }

    #[test]
    fn debug_redacts_verifier_and_key() {
        let p = Pkce::from_parts("secret-verifier".into(), "secret-key".into());
        let s = format!("{p:?}");
        assert!(!s.contains("secret-verifier"));
        assert!(!s.contains("secret-key"));
        assert_eq!(s.matches("<redacted>").count(), 2);
    }
}
