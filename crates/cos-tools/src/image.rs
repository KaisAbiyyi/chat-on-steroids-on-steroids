use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageFormat, ImageReader, Limits};
use serde_json::{Value, json};
use std::io::{self, Cursor};
pub const MAX_BYTES: usize = ((8 * 1024 * 1024 - 64 * 1024) * 3) / 4;
pub fn validate(bytes: &[u8]) -> io::Result<&'static str> {
    let error = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "unable to process image: invalid or unsupported image data",
        )
    };
    if bytes.len() > MAX_BYTES {
        return Err(error());
    }
    let format = image::guess_format(bytes).map_err(|_| error())?;
    let mime = match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Gif => "image/gif",
        ImageFormat::WebP => "image/webp",
        _ => return Err(error()),
    };
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    limits.max_image_width = Some(16 * 1024 * 1024);
    limits.max_image_height = Some(16 * 1024 * 1024);
    reader.limits(limits);
    let (w, h) = ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|_| error())?;
    if u64::from(w) * u64::from(h) > 16 * 1024 * 1024 {
        return Err(error());
    }
    reader.decode().map_err(|_| error())?;
    Ok(mime)
}
pub fn content(bytes: &[u8]) -> io::Result<Value> {
    Ok(json!({"type":"image","data":STANDARD.encode(bytes),"mimeType":validate(bytes)?}))
}
