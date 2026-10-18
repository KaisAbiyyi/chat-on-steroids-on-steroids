//! Isolated tool surfaces and authenticated local ingress.
pub mod code;
pub mod control;
mod find;
pub mod http;
pub mod kernel;
mod patch;
pub mod protocol;
mod read;
pub mod schema;
mod terminal;
pub mod tunnel;
pub mod worker;
use serde_json::Value;
pub fn image_content(value: &Value) -> Result<Value, String> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let (data, mime) = if let Some(url) = value.as_str().or_else(|| value["image_url"].as_str()) {
        let url = url.strip_prefix("data:").ok_or("IMAGE_INVALID")?;
        let (mime, data) = url.split_once(";base64,").ok_or("IMAGE_INVALID")?;
        (data, mime)
    } else if value["type"] == "image" {
        (
            value["data"].as_str().ok_or("IMAGE_INVALID")?,
            value["mimeType"].as_str().ok_or("IMAGE_INVALID")?,
        )
    } else {
        return Err("IMAGE_INVALID".into());
    };
    if data.len() > cos_tools::image::MAX_BYTES.div_ceil(3) * 4 {
        return Err("IMAGE_INVALID".into());
    }
    let bytes = STANDARD.decode(data).map_err(|_| "IMAGE_INVALID")?;
    if STANDARD.encode(&bytes) != data {
        return Err("IMAGE_INVALID".into());
    }
    let actual = cos_tools::image::validate(&bytes).map_err(|_| "IMAGE_INVALID")?;
    if mime != actual && mime != "application/octet-stream" {
        return Err("IMAGE_INVALID".into());
    }
    cos_tools::image::content(&bytes).map_err(|_| "IMAGE_INVALID".into())
}
