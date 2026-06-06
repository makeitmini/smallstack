use hyper::{Response, StatusCode};
use http_body_util::Full;
use hyper::body::Bytes;
use serde::Serialize;

use crate::error::ServeError;
use crate::handler::ResponseBody;

pub fn json<T: Serialize>(status: StatusCode, value: &T) -> Result<Response<ResponseBody>, ServeError> {
    let body = serde_json::to_string(value)
        .map_err(|e| ServeError::new(500, format!("failed to serialize response: {e}")))?;
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(body)))
        .map_err(|e| ServeError::new(500, format!("failed to build response: {e}")))
}

pub fn redirect(location: &str) -> Response<ResponseBody> {
    Response::builder()
        .status(StatusCode::FOUND)
        .header("location", location)
        .body(Full::new(Bytes::new()))
        .unwrap()
}

pub fn empty(status: StatusCode) -> Response<ResponseBody> {
    Response::builder()
        .status(status)
        .body(Full::new(Bytes::new()))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_response_has_correct_status_and_content_type() {
        let resp = json(StatusCode::CREATED, &json!({"id": 1})).unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "application/json"
        );
    }

    #[test]
    fn redirect_response_has_302_and_location() {
        let resp = redirect("/login");
        assert_eq!(resp.status(), StatusCode::FOUND);
        assert_eq!(resp.headers().get("location").unwrap(), "/login");
    }

    #[test]
    fn empty_response_has_correct_status() {
        let resp = empty(StatusCode::NO_CONTENT);
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }
}
