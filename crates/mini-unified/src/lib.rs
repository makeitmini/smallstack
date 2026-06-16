use std::path::PathBuf;
use std::sync::Arc;

use mini_serve::{handler, Handler, RouteBuilder, State};

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
        let h = static_handler::<S>(dir);
        self.get("/", h.clone()).get("/*", h)
    }
}
