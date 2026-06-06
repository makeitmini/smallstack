use std::sync::Arc;

use crate::handler::Handler;

pub type Middleware<S> =
    Arc<dyn Fn(Handler<S>) -> Handler<S> + Send + Sync>;

pub fn middleware<S, F>(f: F) -> Middleware<S>
where
    S: Clone + Send + Sync + 'static,
    F: Fn(Handler<S>) -> Handler<S> + Send + Sync + 'static,
{
    Arc::new(f)
}
