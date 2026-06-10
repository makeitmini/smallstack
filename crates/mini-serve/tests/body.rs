use hyper::Response;
use hyper::body::Bytes;
use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};
use mini_serve::{handler, json_body, RouteBuilder, ServeError};
use std::fmt;

struct CreateUser {
    name:  String,
    email: String,
}

struct CreateUserVisitor;

impl<'de> Visitor<'de> for CreateUserVisitor {
    type Value = CreateUser;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON object with name and email fields")
    }

    fn visit_map<V: MapAccess<'de>>(self, mut map: V) -> Result<CreateUser, V::Error> {
        let mut name: Option<String> = None;
        let mut email: Option<String> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "name" => name = Some(map.next_value()?),
                "email" => email = Some(map.next_value()?),
                _ => {
                    let _: de::IgnoredAny = map.next_value()?;
                }
            }
        }

        Ok(CreateUser {
            name: name.ok_or_else(|| de::Error::missing_field("name"))?,
            email: email.ok_or_else(|| de::Error::missing_field("email"))?,
        })
    }
}

impl<'de> Deserialize<'de> for CreateUser {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_struct("CreateUser", &["name", "email"], CreateUserVisitor)
    }
}

async fn handle_create(
    req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let user: CreateUser = json_body(req).await?;
    let body = serde_json::json!({ "name": user.name, "email": user.email });
    Ok(Response::new(mini_serve::body(Bytes::from(serde_json::to_string(&body).unwrap()))))
}

async fn handle_echo_json(
    req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let value: serde_json::Value = json_body(req).await?;
    Ok(Response::new(mini_serve::body(Bytes::from(serde_json::to_string(&value).unwrap()))))
}

#[tokio::test]
async fn body_size_limit_rejects_oversized_body() {
    let port = RouteBuilder::stateless()
        .post("/echo", handler(handle_echo_json))
        .with_max_body_size(50)
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();

    // Body larger than configured limit → 413
    let resp = client
        .post(format!("http://localhost:{port}/echo"))
        .body(serde_json::json!({"data": "x".repeat(200)}).to_string())
        .header("content-type", "application/json")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 413, "oversized body must be rejected");

    // Small body within limit → still works
    let resp = client
        .post(format!("http://localhost:{port}/echo"))
        .json(&serde_json::json!({"ok": true}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "small body must still succeed");
}

#[tokio::test]
async fn valid_json_body_is_deserialized() {
    let port = RouteBuilder::stateless()
        .post("/users", handler(handle_create))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://localhost:{port}/users"))
        .json(&serde_json::json!({ "name": "Alice", "email": "alice@example.com" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "Alice");
    assert_eq!(body["email"], "alice@example.com");
}

#[tokio::test]
async fn invalid_json_body_returns_400() {
    let port = RouteBuilder::stateless()
        .post("/echo", handler(handle_echo_json))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://localhost:{port}/echo"))
        .body("not-json")
        .header("content-type", "application/json")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn empty_body_returns_400() {
    let port = RouteBuilder::stateless()
        .post("/echo", handler(handle_echo_json))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://localhost:{port}/echo"))
        .body("")
        .header("content-type", "application/json")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn missing_fields_returns_400() {
    let port = RouteBuilder::stateless()
        .post("/users", handler(handle_create))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://localhost:{port}/users"))
        .json(&serde_json::json!({ "name": "Bob" }))  // missing email
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}
