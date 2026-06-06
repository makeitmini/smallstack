use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use hyper::body::Incoming;
use hyper::{Request, Response};
use http_body_util::Full;
use hyper::body::Bytes;

use crate::error::ServeError;
use crate::state::State;

pub type ResponseBody = Full<Bytes>;

pub type Handler<S> = Arc<
    dyn Fn(Request<Incoming>, State<S>)
            -> Pin<Box<dyn Future<Output = Result<Response<ResponseBody>, ServeError>> + Send>>
        + Send
        + Sync,
>;

pub fn handler<S, F, Fut>(f: F) -> Handler<S>
where
    S: Clone + Send + Sync + 'static,
    F: Fn(Request<Incoming>, State<S>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Response<ResponseBody>, ServeError>> + Send + 'static,
{
    Arc::new(move |req, state| Box::pin(f(req, state)))
}
