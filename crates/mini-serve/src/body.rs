use hyper::body::Incoming;
use hyper::Request;
use http_body_util::{BodyExt, LengthLimitError, Limited};
use serde::de::DeserializeOwned;

use crate::error::ServeError;

/// Default maximum request body size (2 MB).
pub const DEFAULT_MAX_BODY_SIZE: usize = 2_097_152;

/// Extension key stored in request extensions by the router
/// to communicate the max body size to body-reading helpers.
#[derive(Clone, Copy, Debug)]
pub struct MaxBodySize(pub usize);

pub async fn json_body<T: DeserializeOwned>(req: Request<Incoming>) -> Result<T, ServeError> {
    let (parts, body) = req.into_parts();
    let max = parts
        .extensions
        .get::<MaxBodySize>()
        .map(|m| m.0)
        .unwrap_or(DEFAULT_MAX_BODY_SIZE);

    // Quick rejection based on Content-Length header (validated again below)
    if let Some(content_length) = parts.headers.get("content-length") {
        if let Ok(s) = content_length.to_str() {
            if let Ok(len) = s.parse::<usize>() {
                if len > max {
                    return Err(ServeError::new(413, "request body too large"));
                }
            }
        }
    }

    let limited = Limited::new(body, max);
    let collected = limited
        .collect()
        .await
        .map_err(|e| {
            if e.downcast_ref::<LengthLimitError>().is_some() {
                ServeError::new(413, "request body too large")
            } else {
                ServeError::new(400, "failed to read request body")
            }
        })?;
    let bytes = collected.to_bytes();
    serde_json::from_slice(&bytes)
        .map_err(|_| ServeError::new(400, "invalid json body"))
}
