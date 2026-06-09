use hyper::Request;
use serde::de::DeserializeOwned;

use crate::error::ServeError;
use crate::router::PathParams;

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

pub fn path_params<T: DeserializeOwned, B>(req: &Request<B>) -> Result<T, ServeError> {
    let params = req
        .extensions()
        .get::<PathParams>()
        .ok_or_else(|| ServeError::new(500, "no path params in request extensions"))?;

    let qs: String = {
        let mut pairs: Vec<String> = params
            .0
            .iter()
            .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
            .collect();
        pairs.sort();
        pairs.join("&")
    };

    serde_qs::from_str(&qs).map_err(|_| ServeError::new(400, "invalid path parameters"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::PathParams;
    use serde::Deserialize;
    use std::collections::HashMap;

    #[derive(Deserialize)]
    struct NoInject {
        #[serde(rename = "*")]
        value: String,
    }

    #[test]
    fn wildcard_value_does_not_inject_into_path_params() {
        let mut params = HashMap::new();
        params.insert("*".to_string(), "a=1&b=2".to_string());

        let req = hyper::Request::builder()
            .extension(PathParams(params))
            .body(())
            .unwrap();

        let result: NoInject = path_params(&req).unwrap();
        assert_eq!(result.value, "a=1&b=2");
    }
}

