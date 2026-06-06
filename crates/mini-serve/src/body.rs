use hyper::body::Incoming;
use hyper::Request;
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;

use crate::error::ServeError;

pub async fn json_body<T: DeserializeOwned>(req: Request<Incoming>) -> Result<T, ServeError> {
    let body = req.into_body();
    let collected = body
        .collect()
        .await
        .map_err(|e| ServeError::new(400, format!("failed to read body: {e}")))?;
    let bytes = collected.to_bytes();
    serde_json::from_slice(&bytes)
        .map_err(|e| ServeError::new(400, format!("invalid json body: {e}")))
}
