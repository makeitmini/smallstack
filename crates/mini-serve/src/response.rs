use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::{Response, StatusCode};
use http_body_util::combinators::BoxBody;
use http_body_util::{Full, StreamBody};
use hyper::body::Bytes;
use serde::Serialize;
use futures_core::Stream;

use crate::error::ServeError;
use crate::handler::ResponseBody;

pub struct Json<T: Serialize>(pub T);

impl<T: Serialize> Json<T> {
    pub fn into_response(self) -> Result<Response<ResponseBody>, ServeError> {
        self.into_response_with_status(StatusCode::OK)
    }

    pub fn into_response_with_status(
        self,
        status: StatusCode,
    ) -> Result<Response<ResponseBody>, ServeError> {
        let body = serde_json::to_string(&self.0)
            .map_err(|e| ServeError::new(500, format!("failed to serialize response: {e}")))?;
        Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(BoxBody::new(Full::new(Bytes::from(body))))
            .map_err(|e| ServeError::new(500, format!("failed to build response: {e}")))
    }
}

pub fn json<T: Serialize>(status: StatusCode, value: &T) -> Result<Response<ResponseBody>, ServeError> {
    let body = serde_json::to_string(value)
        .map_err(|e| ServeError::new(500, format!("failed to serialize response: {e}")))?;
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(BoxBody::new(Full::new(Bytes::from(body))))
        .map_err(|e| ServeError::new(500, format!("failed to build response: {e}")))
}

pub fn redirect(location: &str) -> Response<ResponseBody> {
    Response::builder()
        .status(StatusCode::FOUND)
        .header("location", location)
        .body(BoxBody::new(Full::new(Bytes::new())))
        .unwrap()
}

pub fn empty(status: StatusCode) -> Response<ResponseBody> {
    Response::builder()
        .status(status)
        .body(BoxBody::new(Full::new(Bytes::new())))
        .unwrap()
}

pub fn sse_stream<S>(events: S) -> Response<ResponseBody>
where
    S: Stream<Item = String> + Send + Sync + Unpin + 'static,
{
    let body = BoxBody::new(StreamBody::new(SseStream(events)));
    Response::builder()
        .header("content-type", "text/event-stream")
        .body(body)
        .unwrap()
}

struct SseStream<S>(S);

impl<S> Stream for SseStream<S>
where
    S: Stream<Item = String> + Unpin,
{
    type Item = Result<hyper::body::Frame<Bytes>, Infallible>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let inner = Pin::new(&mut self.get_mut().0);
        match inner.poll_next(cx) {
            Poll::Ready(Some(s)) => {
                Poll::Ready(Some(Ok(hyper::body::Frame::data(Bytes::from(format!("data: {s}\n\n"))))))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
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

    #[tokio::test]
    async fn json_wrapper_round_trips_through_serialization() {
        #[derive(Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Payload {
            name: String,
            count: u32,
        }

        let payload = Payload {
            name: "test".into(),
            count: 7,
        };
        let resp = Json(&payload).into_response().unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "application/json"
        );
        let bytes = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .unwrap()
            .to_bytes();
        let recovered: Payload = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(recovered, payload);
    }

    #[test]
    fn json_wrapper_with_custom_status_uses_provided_status() {
        let resp = Json(42).into_response_with_status(StatusCode::CREATED).unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }
}
