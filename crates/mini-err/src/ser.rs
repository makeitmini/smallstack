use crate::Error;
use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::fmt;
use std::sync::{Mutex, OnceLock};

// Serialized form:
// { "scope": "parse", "kind": "bad", "message": "missing field 'name'", "code": 400 }
//
// Io variant additionally carries "io_kind": "NotFound" (stable string).

pub const MAX_SCOPES: usize = 1024;

static POOL: OnceLock<Mutex<Vec<&'static str>>> = OnceLock::new();

/// Global interner for deserialized scope strings.
///
/// Each unique scope string is leaked at most once and reused thereafter,
/// bounding the per-process leak to the number of distinct scope values
/// ever deserialized (not per-request).
fn intern_scope(s: &str) -> &'static str {
    let mut pool = POOL
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("scope interner lock");
    if let Some(&existing) = pool.iter().find(|&&e| e == s) {
        return existing;
    }
    if pool.len() >= MAX_SCOPES {
        return "overflow";
    }
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    pool.push(leaked);
    leaked
}

pub fn interned_len() -> usize {
    if let Some(mtx) = POOL.get() {
        mtx.lock().expect("scope interner lock").len()
    } else {
        0
    }
}

#[doc(hidden)]
pub mod test_support {
    pub use super::{interned_len, MAX_SCOPES};
}

fn io_kind_to_str(kind: std::io::ErrorKind) -> &'static str {
    match kind {
        std::io::ErrorKind::NotFound => "NotFound",
        std::io::ErrorKind::PermissionDenied => "PermissionDenied",
        std::io::ErrorKind::ConnectionRefused => "ConnectionRefused",
        std::io::ErrorKind::ConnectionReset => "ConnectionReset",
        std::io::ErrorKind::ConnectionAborted => "ConnectionAborted",
        std::io::ErrorKind::NotConnected => "NotConnected",
        std::io::ErrorKind::AddrInUse => "AddrInUse",
        std::io::ErrorKind::AddrNotAvailable => "AddrNotAvailable",
        std::io::ErrorKind::BrokenPipe => "BrokenPipe",
        std::io::ErrorKind::AlreadyExists => "AlreadyExists",
        std::io::ErrorKind::WouldBlock => "WouldBlock",
        std::io::ErrorKind::InvalidInput => "InvalidInput",
        std::io::ErrorKind::InvalidData => "InvalidData",
        std::io::ErrorKind::TimedOut => "TimedOut",
        std::io::ErrorKind::WriteZero => "WriteZero",
        std::io::ErrorKind::Interrupted => "Interrupted",
        std::io::ErrorKind::UnexpectedEof => "UnexpectedEof",
        _ => "Other",
    }
}

fn str_to_io_kind(s: &str) -> std::io::ErrorKind {
    match s {
        "NotFound" => std::io::ErrorKind::NotFound,
        "PermissionDenied" => std::io::ErrorKind::PermissionDenied,
        "ConnectionRefused" => std::io::ErrorKind::ConnectionRefused,
        "ConnectionReset" => std::io::ErrorKind::ConnectionReset,
        "ConnectionAborted" => std::io::ErrorKind::ConnectionAborted,
        "NotConnected" => std::io::ErrorKind::NotConnected,
        "AddrInUse" => std::io::ErrorKind::AddrInUse,
        "AddrNotAvailable" => std::io::ErrorKind::AddrNotAvailable,
        "BrokenPipe" => std::io::ErrorKind::BrokenPipe,
        "AlreadyExists" => std::io::ErrorKind::AlreadyExists,
        "WouldBlock" => std::io::ErrorKind::WouldBlock,
        "InvalidInput" => std::io::ErrorKind::InvalidInput,
        "InvalidData" => std::io::ErrorKind::InvalidData,
        "TimedOut" => std::io::ErrorKind::TimedOut,
        "WriteZero" => std::io::ErrorKind::WriteZero,
        "Interrupted" => std::io::ErrorKind::Interrupted,
        "UnexpectedEof" => std::io::ErrorKind::UnexpectedEof,
        _ => std::io::ErrorKind::Other,
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Error", 5)?;
        state.serialize_field("scope", self.scope())?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.message())?;
        state.serialize_field("code", &self.code())?;
        let io_kind = match self {
            Error::Io { cause, .. } => io_kind_to_str(cause.kind()),
            _ => "",
        };
        state.serialize_field("io_kind", io_kind)?;
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
        let mut code: Option<u16> = None;
        let mut io_kind: Option<String> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "scope" => scope = Some(map.next_value()?),
                "kind" => kind = Some(map.next_value::<String>()?),
                "message" => message = Some(map.next_value()?),
                "code" => code = Some(map.next_value::<u16>()?),
                "io_kind" => io_kind = Some(map.next_value::<String>()?),
                _ => {
                    let _: de::IgnoredAny = map.next_value()?;
                }
            }
        }

        let scope = scope.unwrap_or_default();
        let kind = kind.unwrap_or_default();
        let message = message.unwrap_or_default();
        let code = code;

        let scope: &'static str = intern_scope(&scope);

        let err = match kind.as_str() {
            "io" => {
                let ik = io_kind.as_deref().unwrap_or("Other");
                Error::Io {
                    cause: std::io::Error::new(str_to_io_kind(ik), message),
                    scope,
                    msg: None,
                }
            }
            "net" => Error::Net { msg: message, scope },
            "cfg" => Error::Cfg { msg: message, scope },
            "bad" => Error::Bad { msg: message, scope },
            "gone" => Error::Gone { msg: message, scope },
            _ => Error::Bad { msg: message, scope },
        };

        if let Some(expected) = code {
            let actual = err.code();
            if expected != actual {
                return Err(de::Error::custom(format!(
                    "code mismatch: expected {}, actual {} for kind '{}'",
                    expected, actual, kind
                )));
            }
        }

        Ok(err)
    }
}

impl<'de> Deserialize<'de> for Error {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_struct(
            "Error",
            &["scope", "kind", "message", "code", "io_kind"],
            ErrorVisitor,
        )
    }
}
