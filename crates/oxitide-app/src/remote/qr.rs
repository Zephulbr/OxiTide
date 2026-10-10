//! The pairing URL and its QR code (spec §6.2 step 1). The token rides in the
//! fragment, which browsers never send to a server.

use std::net::IpAddr;

use qrcode::render::svg;
use qrcode::QrCode;

/// `http://<ip>:<port>/remote` (IPv6 bracketed).
pub fn base_url(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(v4) => format!("http://{v4}:{port}/remote"),
        IpAddr::V6(v6) => format!("http://[{v6}]:{port}/remote"),
    }
}

pub fn pairing_url(ip: IpAddr, port: u16, token: &str) -> String {
    format!("{}#p={token}", base_url(ip, port))
}

/// A self-contained SVG (black on white, quiet zone included) so it scans in
/// dark mode too.
pub fn qr_svg(text: &str) -> Result<String, String> {
    let code = QrCode::new(text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(code
        .render::<svg::Color<'_>>()
        .min_dimensions(220, 220)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_carry_the_token_in_the_fragment() {
        let ip: IpAddr = "192.168.1.5".parse().unwrap();
        assert_eq!(pairing_url(ip, 47820, "abc"), "http://192.168.1.5:47820/remote#p=abc");
        let v6: IpAddr = "fe80::1".parse().unwrap();
        assert_eq!(base_url(v6, 47820), "http://[fe80::1]:47820/remote");
    }

    #[test]
    fn qr_is_an_svg() {
        let svg = qr_svg("http://192.168.1.5:47820/remote#p=abcdefghijklmnopqrstuv").unwrap();
        assert!(svg.contains("<svg"), "{svg}");
        assert!(svg.contains("#000000"));
    }
}
