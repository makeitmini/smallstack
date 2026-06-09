use std::future::Future;
use std::pin::Pin;

use hyper::body::Bytes;
use hyper::Response;
use http_body_util::combinators::BoxBody;

use crate::error::StaticError;

pub type ResponseBody = BoxBody<Bytes, StaticError>;

pub struct RequestInfo {
    pub method: String,
    pub path:   String,
}

pub trait Handler: Send + Sync + 'static {
    fn handle(&self, info: RequestInfo) -> Pin<Box<dyn Future<Output = Option<Response<ResponseBody>>> + Send + '_>>;
}
