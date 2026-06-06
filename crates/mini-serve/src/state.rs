use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone)]
pub struct State<S>(Arc<S>);

impl<S> State<S> {
    pub fn new(state: S) -> Self {
        State(Arc::new(state))
    }

    pub fn inner(&self) -> S
    where
        S: Clone,
    {
        S::clone(&self.0)
    }
}

impl<S> Deref for State<S> {
    type Target = S;

    fn deref(&self) -> &S {
        &self.0
    }
}
