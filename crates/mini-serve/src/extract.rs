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
    use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};
    use std::collections::HashMap;
    use std::fmt;

    struct NoInject {
        value: String,
    }

    struct NoInjectVisitor;

    impl<'de> Visitor<'de> for NoInjectVisitor {
        type Value = NoInject;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a JSON object with a * field")
        }

        fn visit_map<V: MapAccess<'de>>(self, mut map: V) -> Result<NoInject, V::Error> {
            let mut value: Option<String> = None;

            while let Some(key) = map.next_key::<String>()? {
                if key == "*" {
                    value = Some(map.next_value()?);
                } else {
                    let _: de::IgnoredAny = map.next_value()?;
                }
            }

            Ok(NoInject {
                value: value.ok_or_else(|| de::Error::missing_field("*"))?,
            })
        }
    }

    impl<'de> Deserialize<'de> for NoInject {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            deserializer.deserialize_struct("NoInject", &["*"], NoInjectVisitor)
        }
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

