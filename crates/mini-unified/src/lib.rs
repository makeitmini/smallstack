use std::path::PathBuf;
use std::sync::Arc;

use mini_serve::{handler, Handler, RouteBuilder, State};

#[cfg(debug_assertions)]
use std::pin::Pin;
#[cfg(debug_assertions)]
use std::task::{Context, Poll};
#[cfg(debug_assertions)]
use futures_core::Stream;
#[cfg(debug_assertions)]
use http_body::Frame;
#[cfg(debug_assertions)]
use http_body_util::combinators::BoxBody;
#[cfg(debug_assertions)]
use http_body_util::StreamBody;
#[cfg(debug_assertions)]
use hyper::body::Bytes;
#[cfg(debug_assertions)]
use mini_serve::ServeError;
#[cfg(debug_assertions)]
use tokio::sync::mpsc::UnboundedReceiver;

/// Returns a mini-serve handler that serves static files from `dir`.
///
/// Use directly when you need a custom route or multiple static roots:
///
/// ```rust,ignore
/// .get("/assets/*", mini_unified::static_handler("./assets"))
/// ```
pub fn static_handler<S: Clone + Send + Sync + 'static>(
    dir: impl Into<PathBuf>,
) -> Handler<S> {
    let dir = Arc::new(dir.into());
    handler(move |req, _state: State<S>| {
        let dir = Arc::clone(&dir);
        async move {
            Ok(mini_static::handle_request(req, &dir, &[], None).await)
        }
    })
}

/// Extension trait that adds [`serve_static`](StaticRouteBuilderExt::serve_static)
/// to [`RouteBuilder`].
///
/// Registers both `"/"` (bare domain) and `"/*"` (deeper paths) so that the
/// bare address serves `index.html` and sub-paths are handled by the static
/// file server.
///
/// ```rust,ignore
/// use mini_unified::StaticRouteBuilderExt;
///
/// RouteBuilder::stateless()
///     .get("/api/users", list_users)
///     .serve_static("./public")
///     .seal();
/// ```
pub trait StaticRouteBuilderExt<S> {
    fn serve_static(self, dir: impl Into<PathBuf>) -> Self;
}

impl<S: Clone + Send + Sync + 'static> StaticRouteBuilderExt<S> for RouteBuilder<S> {
    fn serve_static(self, dir: impl Into<PathBuf>) -> Self {
        let dir: PathBuf = dir.into();
        let h = static_handler::<S>(dir.clone());
        let builder = self.get("/", h.clone()).get("/*", h);

        #[cfg(debug_assertions)]
        let builder = {
            let dir = Arc::new(dir);
            let broadcaster = mini_static::Broadcaster::new();
            mini_static::start_poller(dir, broadcaster.clone());

            let reload_handler = handler(move |_req, _state: State<S>| {
                let broadcaster = broadcaster.clone();
                async move {
                    let rx = broadcaster.subscribe();
                    let stream = ReloadStream { rx };
                    let body = BoxBody::new(StreamBody::new(stream));
                    let resp = hyper::Response::builder()
                        .status(hyper::StatusCode::OK)
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("connection", "keep-alive")
                        .body(body)
                        .map_err(|e| ServeError::new(500, format!("SSE response: {e}")))?;
                    Ok(resp)
                }
            });
            builder.get("/__mini_reload", reload_handler)
        };

        builder
    }
}

#[cfg(debug_assertions)]
struct ReloadStream {
    rx: UnboundedReceiver<mini_static::ReloadEvent>,
}

#[cfg(debug_assertions)]
impl Stream for ReloadStream {
    type Item = Result<Frame<Bytes>, std::convert::Infallible>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match this.rx.poll_recv(cx) {
            Poll::Ready(Some(event)) => {
                let data = serde_json::json!({"type": event.change_type.as_str()});
                let msg = format!(
                    "event: {}\ndata: {}\n\n",
                    event.change_type.as_str(),
                    data
                );
                Poll::Ready(Some(Ok(Frame::data(Bytes::from(msg)))))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
