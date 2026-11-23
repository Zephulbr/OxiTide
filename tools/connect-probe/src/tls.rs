//! TLS to a TIDAL Connect receiver.
//!
//! Receivers present a leaf certificate issued by "TIDAL TS CA" with no CN and
//! no subjectAltName, and are reached by IP. TIDAL's own controller therefore
//! verifies the chain against TIDAL's CA bundle and skips the name check
//! (`checkServerIdentity: () => null`, `rejectUnauthorized: true`). This does
//! the same: chain and validity are checked, the name is not. A failure stops
//! the probe before anything (tokens included) is sent.

use std::sync::{Arc, Mutex};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, ServerName, TrustAnchor, UnixTime};

/// TIDAL's Connect CA bundle as shipped in TIDAL desktop 2.48.0
/// (`app/main/tidalConnect/websocket.js`): "TIDAL TS CA" and "TIDAL Root CA".
pub const TIDAL_CA_PEM: &str = "-----BEGIN CERTIFICATE-----
MIIFrzCCA5egAwIBAgIQNBdQrsnhXAku8aG8fUI9xTANBgkqhkiG9w0BAQsFADA1
MQswCQYDVQQGEwJubzEOMAwGA1UECgwFVElEQUwxFjAUBgNVBAMMDVRJREFMIFJv
b3QgQ0EwHhcNMjAwMzE3MDgwMzQ2WhcNNDAwMjAxMDkwMzQ2WjAzMQswCQYDVQQG
EwJubzEOMAwGA1UECgwFVElEQUwxFDASBgNVBAMMC1RJREFMIFRTIENBMIICIjAN
BgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAvuBuw3NjdQA2ofGvvOpJnJp89q+f
AWLPoaqGrWaaJuw/4cUFP+kz/SiVuexsBHnbl1D621lq+grT0vfrgL0X5eZXBtKn
rIlX66yXr/RcLCapI4OjDLiQ/kAd0qJKx6QNbie7VOyJTLd9hXDhngtCwTBP91j5
e5hVK33SYQ3wLNF1jubG561Ct/aKNBTO1NrVtk5hOgnTlIMnfuP+kqbhKWRv/oU+
Nq+qfbY39aRnbTgoZ9xOz7vygum+Vkq0R9Bx8KZqbXj8n8OsdxA+/r82DOSY3lma
D2CMbKqqfXctKYE68YYfQpVFDs1laXcZgXlGLKwqBR9bTL+hY8waC9v6TzEBmU+a
PUqdH+14NU5MRarGA4pbCgBpUPW8uAG998hBEmXydlm/d8WUOS+1odywNwAiw3qc
zf9CbkBDtMhSjSIvcFmstX6/nFWWeloXKOAX37ojcIfpR/IDsNVdxIUqTHlwj2xr
mLu8ZbKdkZAl7sAjC1klfA7lBls2ZoIxVmIHU9o8e99YMVcYHnqX0OqLngzL4rra
+ufmBqeolJQHjJuuznXYKz7l3Xz7irVvfzZzJd1d6UgVrCFVbWVFxNMwQsShELip
Xr4ViMxPk4fU5zMACC3pLzJ4mThcImD8+Ym+EIJrDTAL0RN82mG37f6wqXL/noHp
oesYEKpC6FQA8v0CAwEAAaOBvDCBuTASBgNVHRMBAf8ECDAGAQH/AgEAMB8GA1Ud
IwQYMBaAFIqbsL3OGur0wNK2jlhgVKKizg8rMB0GA1UdDgQWBBS3GeQDSguernws
iK7RjGss42v36TAOBgNVHQ8BAf8EBAMCAYYwUwYDVR0fBEwwSjBIoEagRIZCaHR0
cDovL2NybC50aWRhbGhpLmZpL2NybC9kOWI5YjVjMi1hNDJjLTQ2MDQtOGRiNi03
ZTU0Zjk1OGY2Y2IuY3JsMA0GCSqGSIb3DQEBCwUAA4ICAQBZBPGz1OJsHr1NFAPK
swL/NjJK2xK80vwZyiPeog/wJh3HAQEBlj6DUGRKGMZrvg7rj/oUVTa3RQ5hx/tx
jzeHamDDHXSylA2z78wfpzktibiH+4ryHhijpobxvj38tjjVveWYXgH3ge5QIFih
DD+KMRZ14BFJZ77FyJdqStaLxQE+pjRrztuvxbkrxOT1f5/yViumdokNzlT1IbfO
CZFC8M3EOvIjGXWVRpwty+N1RnKW3BCRGN8UNWTVh/+XfHH7dM7a67oGw1/p+k6v
+d+dBzF6Up59xwdKv0mvwtNCf6wiIZgMsniE7QXeDpktCup52/hpD7xNNgp6DZjd
2gaQIJQV+Nq9PCZcdpj/2WNSCxKlAvu9qWkt6dXu9pCgh1EZQK9YERZVR4OqlFUB
wf/3ssiNscV0Y5Ut4dyO2wysMbuGK2cYE46CheoOmh80+Ey9Dkf9QjSeaJZfxab2
x++xZY43/+OcDPKKGJo7INvtAE+m4eHBUQbUrGWoh0hTc5tXrJIS+1CMX66vuuKA
emXuhRzHqiUjwy+558qDTbmDsj1B3fAb4RCXS/zBl9/GxrNDfCVBHYu84j7d2Qtf
7kqX2jY7Sg92tQv4yQQfT395UZCLku4MoaVMZPGb4sFMyg8o7gu0+MBfx/G/dgnP
55wtdVtL3jxezmTavYgqx+qMLA==
-----END CERTIFICATE-----
-----BEGIN CERTIFICATE-----
MIIFNjCCAx6gAwIBAgIQWO2pp96w7ja06H8Vk0VEiDANBgkqhkiG9w0BAQ0FADA1
MQswCQYDVQQGEwJubzEOMAwGA1UECgwFVElEQUwxFjAUBgNVBAMMDVRJREFMIFJv
b3QgQ0EwHhcNMjAwMjA1MTA1NTI1WhcNNDAwMjA1MTE1NTI1WjA1MQswCQYDVQQG
EwJubzEOMAwGA1UECgwFVElEQUwxFjAUBgNVBAMMDVRJREFMIFJvb3QgQ0EwggIi
MA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQCfWPE+WVPtda9FP6uIS4l0A8vh
+VsmeUbq7hk9YZmA6HqNm8Lp2YjKBrW/rzDyK7zYF+Qe3/R9+cS0dCXQB1huVTG7
LklY8dY1VwLSdUwJqe1+GGWm9C1UsUU+BZlfFQuFYECb/aQpCMOv9+ivybreb5Tj
PSLM5Ba+O9CEC4+23CTfvCadi0h7miGdxMBqtSd5SaiBHBQyOnYTKGUekrkms09k
5BuY5DBk1IWHbeLz13T65ZrLrMAuFjVD/q4FHw5IJG1iN1QLZKZfVqO89MB1hxFc
aBVIPD5qCi1/vUwsrHOIYWvJGSqvg7h3vz5OR9T3AZbUcMu/uoPs+5cd8a2kza/0
UVXmgy3AwJRBJfTathTwvczgbAzGgZbbht8Jtjl11eMwJ8XuNmRCpyxVkE/ehI+M
euFn8QCG3YawFryrrAd89XBeiEJ3u+Orjc9V8VO26rHkxjmURgrcC+7w6cV/GlB3
8Mb/18nSiyvxYeg5fUkJjwsD6/yOnD7X0nqspoC554HFiz1zZNgt2iKlX9yuzTr2
336+rIr/ijQR9KvTXTEoSH9XKIoJzGp44AK90GVKsiLQYfEFj1RiA22eqx0T/vKE
y32gYv7bfDuY1q+41oTDGAVC4YNSYvfr/HnxZ3FzuGylQYAvWy28aNk64TJJHZrE
JXfer2/t4M9AwMAAmwIDAQABo0IwQDAPBgNVHRMBAf8EBTADAQH/MB0GA1UdDgQW
BBSKm7C9zhrq9MDSto5YYFSios4PKzAOBgNVHQ8BAf8EBAMCAYYwDQYJKoZIhvcN
AQENBQADggIBAB5mlBJ2MxK77ryz05gp83v2VTV4OfB7DVxfzUZW7eEXnNlhLfFP
X5u+zp3LiAvyt4SpnvTTPHlUzUSYdz5Wiijsd+OO6VQQhma4iDdb7DnIznRBdrZ7
7ahxtebGR2UtTVfRq15KvrWVYTsDOr95nQX88n9gUiCzloahDxYI37FYf7hg0ctC
o56NelbC7zvFcYqPybpabwqaaBINKsV//d0KKAnPY3TWkWOp5lvmYB3wiHKCL16/
rD+uHqbjA8igaAtgrnpiWmxRiajzei9UxIRGGar6JwOWNqbCmPnbBO6TdJqEz4Xs
ylU15u/JMwfeMrCQMsBECdq/R6yxRZ6JRqymndT8c/pwPZ1xdFaaM3CUrE0gMITe
kLU69W2p4Lm1Ul5ST+CoTGfLKwIfeUFSz9k3AG1s8aB3WS5Kjt13cmktuEO2WGRP
iTy0ysjuCVqIoa137/1HepR6NYx7nPQARuC2v2b50rkmXC3wqtYeNuWY8vD0AVyM
VGmriYp/v5UDS0JQCqzStDVzZoJvihssF/Cb9ExUzlYR7nrL09s90UinnGSwZG+l
wt1erm0HKlygg2us7q8xBRTXLJGXcTZ8969C/6uPkKN5s9KHTZrswjXFz+0zLz2y
42V7gw1nqPWH+f7wXqwxhoifvcAHIhNM02lLu6fJat0gQBtivBKEHUaG
-----END CERTIFICATE-----
";

/// Verifies a receiver's chain against [`TIDAL_CA_PEM`] without a name check,
/// and remembers what the peer presented and why verification failed.
#[derive(Debug)]
pub struct TidalCaVerifier {
    anchors: Vec<TrustAnchor<'static>>,
    provider: Arc<CryptoProvider>,
    presented: Mutex<Vec<CertificateDer<'static>>>,
    failure: Mutex<Option<String>>,
}

impl TidalCaVerifier {
    pub fn new(provider: Arc<CryptoProvider>) -> anyhow::Result<Self> {
        let mut anchors = Vec::new();
        for cert in CertificateDer::pem_slice_iter(TIDAL_CA_PEM.as_bytes()) {
            let cert = cert?;
            anchors.push(webpki::anchor_from_trusted_cert(&cert)?.to_owned());
        }
        anyhow::ensure!(anchors.len() == 2, "expected two TIDAL CA certificates");
        Ok(Self {
            anchors,
            provider,
            presented: Mutex::new(Vec::new()),
            failure: Mutex::new(None),
        })
    }

    /// The chain the last peer presented (leaf first).
    pub fn presented(&self) -> Vec<CertificateDer<'static>> {
        self.presented.lock().unwrap().clone()
    }

    pub fn failure(&self) -> Option<String> {
        self.failure.lock().unwrap().clone()
    }

    fn check(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> Result<(), webpki::Error> {
        let cert = webpki::EndEntityCert::try_from(end_entity)?;
        cert.verify_for_usage(
            self.provider.signature_verification_algorithms.all,
            &self.anchors,
            intermediates,
            now,
            webpki::KeyUsage::server_auth(),
            None,
            None,
        )?;
        Ok(())
    }
}

impl ServerCertVerifier for TidalCaVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        {
            let mut presented = self.presented.lock().unwrap();
            presented.clear();
            presented.push(end_entity.clone().into_owned());
            presented.extend(intermediates.iter().map(|c| c.clone().into_owned()));
        }
        match self.check(end_entity, intermediates, now) {
            Ok(()) => Ok(ServerCertVerified::assertion()),
            Err(e) => {
                let message = format!("receiver certificate rejected: {e:?}");
                *self.failure.lock().unwrap() = Some(message.clone());
                Err(rustls::Error::General(message))
            }
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// A client config that trusts only TIDAL's Connect CA, plus the verifier so
/// the caller can report what the peer presented.
pub fn client_config() -> anyhow::Result<(Arc<ClientConfig>, Arc<TidalCaVerifier>)> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(TidalCaVerifier::new(provider.clone())?);
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(verifier.clone())
        .with_no_client_auth();
    Ok((Arc::new(config), verifier))
}

/// PEM text for a DER certificate, for the log when verification fails.
pub fn to_pem(cert: &CertificateDer<'_>) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(cert.as_ref());
    let mut out = String::from("-----BEGIN CERTIFICATE-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        out.push('\n');
    }
    out.push_str("-----END CERTIFICATE-----");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_bundle_parses_into_two_anchors() {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = TidalCaVerifier::new(provider).unwrap();
        assert_eq!(verifier.anchors.len(), 2);
    }

    #[test]
    fn a_ca_certificate_is_not_accepted_as_a_server_leaf() {
        // The TS CA itself is not a valid end-entity for server auth (CA:TRUE).
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = TidalCaVerifier::new(provider).unwrap();
        let ts_ca = CertificateDer::pem_slice_iter(TIDAL_CA_PEM.as_bytes())
            .next()
            .unwrap()
            .unwrap();
        assert!(verifier.check(&ts_ca, &[], UnixTime::now()).is_err());
    }
}
