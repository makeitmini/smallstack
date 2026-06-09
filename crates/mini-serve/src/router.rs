use std::collections::HashMap;

use hyper::Method;

use crate::handler::Handler;

/// Path parameter storage — inserted into request extensions during routing.
#[derive(Clone, Debug, Default)]
pub struct PathParams(pub HashMap<String, String>);

/// Query parameter storage — inserted into request extensions during routing.
#[derive(Clone, Debug, Default)]
pub struct QueryParams(pub HashMap<String, String>);

#[derive(Default)]
pub struct Router<S> {
    root: Node<S>,
}

struct Node<S> {
    segment:   String,
    param_name: String,
    is_wildcard: bool,
    handlers:   HashMap<Method, Handler<S>>,
    children:   Vec<Node<S>>,
}

impl<S> Default for Node<S> {
    fn default() -> Self {
        Node {
            segment:    String::new(),
            param_name: String::new(),
            is_wildcard: false,
            handlers:   HashMap::new(),
            children:   Vec::new(),
        }
    }
}

impl<S: Clone + Send + Sync + 'static> Router<S> {
    pub fn new() -> Self {
        Router { root: Node::default() }
    }

    pub fn insert(&mut self, method: Method, path: &str, handler: Handler<S>) {
        let segments = split_path(path);
        let mut node = &mut self.root;

        for seg in segments {
            if seg == "*" {
                // Find or create wildcard child
                if let Some(idx) = node.children.iter().position(|c| c.is_wildcard) {
                    node = &mut node.children[idx];
                } else {
                    node.children.push(Node {
                        segment:    "*".to_string(),
                        param_name: String::new(),
                        is_wildcard: true,
                        handlers:   HashMap::new(),
                        children:   Vec::new(),
                    });
                    node = node.children.last_mut().unwrap();
                }
            } else if let Some(param_name) = seg.strip_prefix(':') {
                // Find or create param child
                if let Some(idx) = node.children.iter().position(|c| c.param_name == param_name) {
                    node = &mut node.children[idx];
                } else {
                    node.children.push(Node {
                        segment:    seg.to_string(),
                        param_name: param_name.to_string(),
                        is_wildcard: false,
                        handlers:   HashMap::new(),
                        children:   Vec::new(),
                    });
                    node = node.children.last_mut().unwrap();
                }
            } else {
                // Static segment — find or create
                if let Some(idx) = node.children.iter().position(|c| c.segment == seg) {
                    node = &mut node.children[idx];
                } else {
                    node.children.push(Node {
                        segment:    seg.to_string(),
                        param_name: String::new(),
                        is_wildcard: false,
                        handlers:   HashMap::new(),
                        children:   Vec::new(),
                    });
                    node = node.children.last_mut().unwrap();
                }
            }
        }

        node.handlers.insert(method, handler);
    }

    /// Returns true if any route is registered for the given path (ignoring method).
    pub fn has_path(&self, path: &str) -> bool {
        let segments = split_path(path);
        let mut params = PathParams::default();
        Self::find_node(&self.root, &segments, 0, &mut params)
            .map_or(false, |n| !n.handlers.is_empty())
    }

    pub fn match_route<'a>(
        &'a self,
        method: &Method,
        path: &str,
    ) -> Option<(&'a Handler<S>, PathParams)> {
        let segments = split_path(path);
        let mut params = PathParams::default();
        let node = Self::find_node(&self.root, &segments, 0, &mut params)?;
        node.handlers.get(method).map(|h| (h, params))
    }

    /// Recursive backtracking search for a matching trie node.
    ///
    /// Tries children in precedence order: static → param → wildcard.
    /// On a dead end it backtracks to try the next-choice child so that
    /// overlapping routes like `/a/b` and `/:x/c` both work (e.g. `/a/c`
    /// falls back from the `a` subtree to `:x`).
    fn find_node<'a>(
        node: &'a Node<S>,
        segments: &[String],
        idx: usize,
        params: &mut PathParams,
    ) -> Option<&'a Node<S>> {
        if idx == segments.len() {
            return Some(node);
        }

        let seg = &segments[idx];

        // 1. Static children — exact segment match
        for child in &node.children {
            if !child.is_wildcard && child.param_name.is_empty() && child.segment == *seg {
                let mut p = params.clone();
                if let Some(found) = Self::find_node(child, segments, idx + 1, &mut p) {
                    *params = p;
                    return Some(found);
                }
            }
        }

        // 2. Param children — captures segment as named parameter
        for child in &node.children {
            if !child.is_wildcard && !child.param_name.is_empty() {
                let mut p = params.clone();
                p.0.insert(child.param_name.clone(), seg.clone());
                if let Some(found) = Self::find_node(child, segments, idx + 1, &mut p) {
                    *params = p;
                    return Some(found);
                }
            }
        }

        // 3. Wildcard child — captures all remaining segments
        for child in &node.children {
            if child.is_wildcard {
                params.0.insert("*".to_string(), segments[idx..].join("/"));
                return Some(child);
            }
        }

        None
    }

    pub fn apply_middleware(&mut self, middleware: &[crate::middleware::Middleware<S>]) {
        for m in middleware {
            self.root.apply_middleware(m);
        }
    }
}

impl<S> Node<S> {
    fn apply_middleware(&mut self, m: &crate::middleware::Middleware<S>) {
        for handler in self.handlers.values_mut() {
            *handler = m(handler.clone());
        }
        for child in &mut self.children {
            child.apply_middleware(m);
        }
    }
}

fn split_path(path: &str) -> Vec<String> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

// Inline tests OK per STANDARDS.md: Router is a pure-logic module
// with no public API surface (Router/Node are crate-private).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ServeError;
    use hyper::Response;
    use hyper::body::Bytes;

    fn dummy_handler() -> Handler<()> {
        crate::handler::handler(|_, _| async {
            Ok::<_, ServeError>(Response::new(crate::handler::body(Bytes::from("ok"))))
        })
    }

    #[test]
    fn insert_and_match_static() {
        let mut router = Router::new();
        router.insert(Method::GET, "/hello", dummy_handler());
        let (_, _) = router.match_route(&Method::GET, "/hello").unwrap();
    }

    #[test]
    fn match_with_path_param() {
        let mut router = Router::new();
        router.insert(Method::GET, "/users/:id", dummy_handler());
        let (_, params) = router.match_route(&Method::GET, "/users/42").unwrap();
        assert_eq!(params.0.get("id").unwrap(), "42");
    }

    #[test]
    fn match_with_wildcard() {
        let mut router = Router::new();
        router.insert(Method::GET, "/files/*", dummy_handler());
        let (_, params) = router.match_route(&Method::GET, "/files/a/b/c").unwrap();
        assert_eq!(params.0.get("*").unwrap(), "a/b/c");
    }

    #[test]
    fn no_match_for_unregistered_route() {
        let mut router = Router::new();
        router.insert(Method::GET, "/hello", dummy_handler());
        assert!(router.match_route(&Method::GET, "/world").is_none());
    }

    #[test]
    fn method_mismatch_returns_none() {
        let mut router = Router::new();
        router.insert(Method::GET, "/hello", dummy_handler());
        assert!(router.match_route(&Method::POST, "/hello").is_none());
    }

    #[test]
    fn root_path_matches() {
        let mut router = Router::new();
        router.insert(Method::GET, "/", dummy_handler());
        let (_, _) = router.match_route(&Method::GET, "/").unwrap();
    }
}
