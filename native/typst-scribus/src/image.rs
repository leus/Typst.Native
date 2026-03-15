//! Image embedding for Scribus export.
//!
//! Raster images are base64-encoded and written as inline image data.

use base64::Engine;
use typst_library::visualize::{ExchangeFormat, Image, ImageKind, RasterFormat};

/// Extract the raw image bytes and MIME type for embedding into SLA.
pub fn image_data_base64(image: &Image) -> Option<(String, &'static str)> {
    match image.kind() {
        ImageKind::Raster(raster) => {
            let data = raster.data();
            let encoded = base64::engine::general_purpose::STANDARD.encode(data.as_slice());
            let mime = match raster.format() {
                RasterFormat::Exchange(ExchangeFormat::Png) => "image/png",
                RasterFormat::Exchange(ExchangeFormat::Jpg) => "image/jpeg",
                RasterFormat::Exchange(ExchangeFormat::Gif) => "image/gif",
                _ => "application/octet-stream",
            };
            Some((encoded, mime))
        }
        // SVG and PDF images are not directly embeddable in SLA.
        _ => None,
    }
}
