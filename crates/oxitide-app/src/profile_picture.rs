//! Preparing a profile picture for TIDAL's artwork upload: a centered square
//! crop, at most 1280 px, re-encoded as JPEG, plus the MD5 TIDAL signs the
//! upload link for (hex in the create call, base64 in `Content-MD5`).

use base64::Engine as _;
use md5::{Digest, Md5};

/// The largest side TIDAL's profile art is served at.
const MAX_SIDE: u32 = 1280;
/// Pictures larger than this are refused before decoding.
pub const MAX_INPUT_BYTES: usize = 20 * 1024 * 1024;

pub struct PreparedPicture {
    pub jpeg: Vec<u8>,
    pub md5_hex: String,
    pub md5_base64: String,
}

/// Decode `bytes` (JPEG or PNG), crop to the centered square, scale down to
/// [`MAX_SIDE`] and encode as JPEG (quality 90).
pub fn prepare(bytes: &[u8]) -> Result<PreparedPicture, String> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err("That picture is too large (20 MB at most).".into());
    }
    let image = image::load_from_memory(bytes)
        .map_err(|_| "That file isn't a picture OxiTide can read (use JPEG or PNG).".to_string())?;
    let side = image.width().min(image.height());
    if side == 0 {
        return Err("That picture is empty.".into());
    }
    let x = (image.width() - side) / 2;
    let y = (image.height() - side) / 2;
    let mut square = image.crop_imm(x, y, side, side);
    if side > MAX_SIDE {
        square = square.resize_exact(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Lanczos3);
    }
    let mut jpeg = Vec::new();
    square
        .to_rgb8()
        .write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90))
        .map_err(|e| format!("could not encode the picture: {e}"))?;
    let digest = Md5::digest(&jpeg);
    let md5_hex = digest.iter().map(|b| format!("{b:02x}")).collect();
    let md5_base64 = base64::engine::general_purpose::STANDARD.encode(digest);
    Ok(PreparedPicture { jpeg, md5_hex, md5_base64 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(width, height, image::Rgb([200, 10, 10]));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn crops_to_a_centered_square_jpeg_with_its_md5() {
        let prepared = prepare(&png(300, 200)).unwrap();
        let out = image::load_from_memory(&prepared.jpeg).unwrap();
        assert_eq!((out.width(), out.height()), (200, 200));
        assert_eq!(image::guess_format(&prepared.jpeg).unwrap(), image::ImageFormat::Jpeg);
        assert_eq!(prepared.md5_hex.len(), 32);
        let raw = base64::engine::general_purpose::STANDARD.decode(&prepared.md5_base64).unwrap();
        let hex: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, prepared.md5_hex);
    }

    #[test]
    fn scales_large_pictures_down_to_1280() {
        let prepared = prepare(&png(1500, 1600)).unwrap();
        let out = image::load_from_memory(&prepared.jpeg).unwrap();
        assert_eq!((out.width(), out.height()), (1280, 1280));
    }

    #[test]
    fn refuses_what_is_not_a_picture() {
        assert!(prepare(b"not an image").is_err());
    }
}
