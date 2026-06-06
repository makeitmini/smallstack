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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use hyper::body::Bytes;
    use http_body_util::Full;

    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct UserPath {
        id:   u32,
        name: String,
    }

    fn mock_request() -> Request<Full<Bytes>> {
        let body = Full::new(Bytes::new());
        Request::new(body)
    }

    #[test]
    fn path_params_deserializes_valid_params() {
        let mut req = mock_request();
        let mut map = HashMap::new();
        map.insert("id".to_string(), "42".to_string());
        map.insert("name".to_string(), "alice".to_string());
        req.extensions_mut().insert(PathParams(map));

        let result: UserPath = path_params(&req).unwrap();
        assert_eq!(result, UserPath { id: 42, name: "alice".to_string() });
    }

    #[test]
    fn path_params_returns_500_when_missing() {
        let req = mock_request();
        let result: Result<UserPath, ServeError> = path_params(&req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, 500);
    }

    #[test]
    fn path_params_returns_400_on_type_mismatch() {
        let mut req = mock_request();
        let mut map = HashMap::new();
        map.insert("id".to_string(), "not-a-number".to_string());
        req.extensions_mut().insert(PathParams(map));

        let result: Result<UserPath, ServeError> = path_params(&req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, 400);
    }
}
