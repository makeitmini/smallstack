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

