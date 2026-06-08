use crate::Error;
use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::fmt;
use std::sync::{Mutex, OnceLock};

// Serialized form:
// { "scope": "parse", "kind": "bad", "message": "missing field 'name'", "code": 400 }

/// Global interner for deserialized scope strings.
///
/// Each unique scope string is leaked at most once and reused thereafter,
/// bounding the per-process leak to the number of distinct scope values
/// ever deserialized (not per-request).
fn intern_scope(s: &str) -> &'static str {
    static POOL: OnceLock<Mutex<Vec<&'static str>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("scope interner lock");
    if let Some(&existing) = pool.iter().find(|&&e| e == s) {
        return existing;
    }
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    pool.push(leaked);
    leaked
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Error", 4)?;
        state.serialize_field("scope", self.scope())?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.message())?;
        state.serialize_field("code", &self.code())?;
        state.end()
    }
}

struct ErrorVisitor;

impl<'de> Visitor<'de> for ErrorVisitor {
    type Value = Error;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON object with scope, kind, message, and code fields")
    }

    fn visit_map<V: MapAccess<'de>>(self, mut map: V) -> Result<Error, V::Error> {
        let mut scope: Option<String> = None;
        let mut kind: Option<String> = None;
        let mut message: Option<String> = None;
        let mut _code: Option<u16> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "scope" => scope = Some(map.next_value()?),
                "kind" => kind = Some(map.next_value::<String>()?),
                "message" => message = Some(map.next_value()?),
                "code" => _code = Some(map.next_value::<u16>()?),
                _ => {
                    let _: de::IgnoredAny = map.next_value()?;
                }
            }
        }

        let scope = scope.unwrap_or_default();
        let kind = kind.unwrap_or_default();
        let message = message.unwrap_or_default();

        let scope: &'static str = intern_scope(&scope);

        Ok(match kind.as_str() {
            "io" => Error::Io {
                cause: std::io::Error::other(message),
                scope,
            },
            "net" => Error::Net { msg: message, scope },
            "cfg" => Error::Cfg { msg: message, scope },
            "bad" => Error::Bad { msg: message, scope },
            "gone" => Error::Gone { msg: message, scope },
            _ => Error::Bad { msg: message, scope },
        })
    }
}

impl<'de> Deserialize<'de> for Error {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_struct("Error", &["scope", "kind", "message", "code"], ErrorVisitor)
    }
}
