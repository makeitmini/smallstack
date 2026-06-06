use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};
use http_body_util::combinators::BoxBody;
use http_body_util::Full;

use crate::error::ServeError;
use crate::state::State;

pub type ResponseBody = BoxBody<Bytes, Infallible>;

pub fn body(bytes: Bytes) -> ResponseBody {
    BoxBody::new(Full::new(bytes))
}

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
