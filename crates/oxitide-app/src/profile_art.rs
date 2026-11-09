//! Preparing a profile's picture, banner or logo for TIDAL's artwork
//! upload, plus the MD5 TIDAL signs the upload link for (hex in the create
//! call, base64 in `Content-MD5`):
//! - picture: a centered square crop, at most 1280 px, JPEG;
//! - banner: its own shape, at most 2560 px wide, JPEG (TIDAL documents no
//!   banner shape, so nothing is cropped);
//! - logo: its own shape, at most 1280 px on the long side, PNG, so its
//!   transparency survives.

use base64::Engine as _;
use image::DynamicImage;
use md5::{Digest, Md5};
use oxitide_tidal::profiles::ProfileArt;

/// Chosen files larger than this are refused before decoding.
pub const MAX_INPUT_BYTES: usize = 20 * 1024 * 1024;
/// TIDAL's editor takes files up to 8 MB; a prepared file must fit.
const MAX_UPLOAD_BYTES: usize = 8 * 1024 * 1024;

pub struct PreparedArt {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
    pub md5_hex: String,
    pub md5_base64: String,
}

/// Scale `image` down (never up) so its width is at most `max_width` and its
/// height at most `max_height`, keeping its shape.
fn fit(image: DynamicImage, max_width: u32, max_height: u32) -> DynamicImage {
    if image.width() <= max_width && image.height() <= max_height {
        return image;
    }
    image.resize(max_width, max_height, image::imageops::FilterType::Lanczos3)
}

fn jpeg(image: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    image
        .to_rgb8()
        .write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
            &mut out, 90,
        ))
        .map_err(|e| format!("could not encode the picture: {e}"))?;
    Ok(out)
}

fn png(image: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut out = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image.to_rgba8())
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| format!("could not encode the picture: {e}"))?;
    Ok(out.into_inner())
}

/// Decode `bytes` (JPEG or PNG) and shape it for `art`.
pub fn prepare(bytes: &[u8], art: ProfileArt) -> Result<PreparedArt, String> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err("That picture is too large (20 MB at most).".into());
    }
    let image = image::load_from_memory(bytes)
        .map_err(|_| "That file isn't a picture OxiTide can read (use JPEG or PNG).".to_string())?;
    if image.width() == 0 || image.height() == 0 {
        return Err("That picture is empty.".into());
    }
    let (bytes, content_type) = match art {
        ProfileArt::Picture => {
            let side = image.width().min(image.height());
            let x = (image.width() - side) / 2;
            let y = (image.height() - side) / 2;
            (
                jpeg(&fit(image.crop_imm(x, y, side, side), 1280, 1280))?,
                "image/jpeg",
            )
        }
        ProfileArt::Banner => (jpeg(&fit(image, 2560, 2560))?, "image/jpeg"),
        ProfileArt::Logo => (png(&fit(image, 1280, 1280))?, "image/png"),
    };
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(
            "That picture is still over TIDAL's 8 MB limit once resized; try a simpler image."
                .into(),
        );
    }
    let digest = Md5::digest(&bytes);
    let md5_hex = digest.iter().map(|b| format!("{b:02x}")).collect();
    let md5_base64 = base64::engine::general_purpose::STANDARD.encode(digest);
    Ok(PreparedArt {
        bytes,
        content_type,
        md5_hex,
        md5_base64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_of(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 10, 10, 128]));
        let mut buf = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn size(bytes: &[u8]) -> (u32, u32) {
        let out = image::load_from_memory(bytes).unwrap();
        (out.width(), out.height())
    }

    #[test]
    fn a_picture_is_a_centered_square_jpeg_with_its_md5() {
        let prepared = prepare(&png_of(300, 200), ProfileArt::Picture).unwrap();
        assert_eq!(size(&prepared.bytes), (200, 200));
        assert_eq!(
            image::guess_format(&prepared.bytes).unwrap(),
            image::ImageFormat::Jpeg
        );
        assert_eq!(prepared.content_type, "image/jpeg");
        assert_eq!(prepared.md5_hex.len(), 32);
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&prepared.md5_base64)
            .unwrap();
        let hex: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, prepared.md5_hex);
    }

    #[test]
    fn a_large_picture_scales_down_to_1280() {
        let prepared = prepare(&png_of(1500, 1600), ProfileArt::Picture).unwrap();
        assert_eq!(size(&prepared.bytes), (1280, 1280));
    }

    #[test]
    fn a_banner_keeps_its_shape_and_is_at_most_2560_wide() {
        let prepared = prepare(&png_of(3000, 1000), ProfileArt::Banner).unwrap();
        assert_eq!(size(&prepared.bytes), (2560, 853));
        assert_eq!(prepared.content_type, "image/jpeg");
        let small = prepare(&png_of(1200, 300), ProfileArt::Banner).unwrap();
        assert_eq!(size(&small.bytes), (1200, 300), "never scaled up");
    }

    #[test]
    fn a_logo_is_a_png_that_keeps_its_transparency() {
        let prepared = prepare(&png_of(1600, 400), ProfileArt::Logo).unwrap();
        assert_eq!(prepared.content_type, "image/png");
        let out = image::load_from_memory(&prepared.bytes).unwrap();
        assert_eq!((out.width(), out.height()), (1280, 320));
        assert_eq!(out.to_rgba8().get_pixel(0, 0)[3], 128);
    }

    #[test]
    fn refuses_what_is_not_a_picture() {
        assert!(prepare(b"not an image", ProfileArt::Banner).is_err());
    }
}
