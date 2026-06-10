use hyper::Response;
use hyper::body::Bytes;
use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};
use mini_serve::{handler, path_params, RouteBuilder, ServeError};
use std::fmt;

struct UserPath {
    id: u32,
}

struct UserPathVisitor;

impl<'de> Visitor<'de> for UserPathVisitor {
    type Value = UserPath;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON object with an id field")
    }

    fn visit_map<V: MapAccess<'de>>(self, mut map: V) -> Result<UserPath, V::Error> {
        let mut id: Option<u32> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "id" => id = Some(map.next_value()?),
                _ => {
                    let _: de::IgnoredAny = map.next_value()?;
                }
            }
        }

        Ok(UserPath {
            id: id.ok_or_else(|| de::Error::missing_field("id"))?,
        })
    }
}

impl<'de> Deserialize<'de> for UserPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_struct("UserPath", &["id"], UserPathVisitor)
    }
}

async fn handle_user(
    req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let path: UserPath = path_params(&req)?;
    let body = serde_json::json!({ "id": path.id });
    Ok(Response::new(mini_serve::body(Bytes::from(serde_json::to_string(&body).unwrap()))))
}

#[tokio::test]
async fn path_param_extracts_typed_value() {
    let port = RouteBuilder::stateless()
        .get("/users/:id", handler(handle_user))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/users/42"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], 42);
}

#[tokio::test]
async fn path_param_with_invalid_type_returns_400() {
    let port = RouteBuilder::stateless()
        .get("/users/:id", handler(handle_user))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/users/abc"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}
