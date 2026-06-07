use hyper::Request;
use serde::de::DeserializeOwned;

use crate::error::ServeError;
use crate::router::PathParams;

pub fn path_params<T: DeserializeOwned, B>(req: &Request<B>) -> Result<T, ServeError> {
    let params = req
        .extensions()
        .get::<PathParams>()
        .ok_or_else(|| ServeError::new(500, "no path params in request extensions"))?;

    let qs: String = {
        let mut pairs: Vec<String> = params
            .0
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        pairs.sort();
        pairs.join("&")
    };

    serde_qs::from_str(&qs).map_err(|e| ServeError::new(400, format!("invalid path params: {e}")))
}

