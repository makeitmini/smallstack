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
            .map_err(|_| ServeError::new(500, "failed to serialize response"))?;
        Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(BoxBody::new(Full::new(Bytes::from(body))))
            .map_err(|_| ServeError::new(500, "failed to build response"))
    }
}

pub fn json<T: Serialize>(status: StatusCode, value: &T) -> Result<Response<ResponseBody>, ServeError> {
    let body = serde_json::to_string(value)
        .map_err(|_| ServeError::new(500, "failed to serialize response"))?;
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(BoxBody::new(Full::new(Bytes::from(body))))
        .map_err(|_| ServeError::new(500, "failed to build response"))
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

pub(crate) fn sse_frame(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for line in s.lines() {
        out.push_str("data: ");
        out.push_str(line);
        out.push('\n');
    }
    out.push('\n');
    out
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
                Poll::Ready(Some(Ok(hyper::body::Frame::data(Bytes::from(sse_frame(&s))))))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_frame_prefixes_each_line_with_data() {
        let result = sse_frame("line1\nevent: pwn");
        assert_eq!(result, "data: line1\ndata: event: pwn\n\n");
    }
}

