use std::path::PathBuf;
use std::sync::Arc;

use mini_serve::{handler, Handler, State};

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
