#![cfg(feature = "serde")]

use mini_err::Error;
use std::error::Error as StdError;

#[test]
fn serialized_bad_error_has_correct_code_field() {
    let err = Error::bad("parse", "missing field 'name'");
    let json = serde_json::to_string(&err).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["code"], 400);
}

#[test]
fn round_trip_through_serde_preserves_all_fields() {
    let err = Error::bad("parse", "missing field 'name'");
    let json = serde_json::to_string(&err).unwrap();
    let deserialized: Error = serde_json::from_str(&json).unwrap();
    assert_eq!(err.scope(), deserialized.scope());
    assert_eq!(err.kind(), deserialized.kind());
    assert_eq!(err.message(), deserialized.message());
    assert_eq!(err.code(), deserialized.code());
}

#[test]
fn serialized_json_has_expected_shape() {
    let err = Error::bad("parse", "missing field 'name'");
    let json = serde_json::to_string(&err).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["scope"], "parse");
    assert_eq!(value["kind"], "bad");
    assert_eq!(value["message"], "missing field 'name'");
    assert_eq!(value["code"], 400);
}

#[test]
fn round_trip_io_variant() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "config.toml");
    let err = Error::Io { cause: io, scope: "fs" };
    let json = serde_json::to_string(&err).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["scope"], "fs");
    assert_eq!(value["kind"], "io");
    assert!(value["message"].as_str().unwrap().contains("config.toml"));
    assert_eq!(value["code"], 500);

    let deserialized: Error = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.scope(), "fs");
    assert_eq!(deserialized.kind(), "io");
    assert_eq!(deserialized.code(), 500);
    assert!(StdError::source(&deserialized).is_some());
}

#[test]
fn round_trip_net_variant() {
    let err = Error::net("upstream", "connection refused");
    let json = serde_json::to_string(&err).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["scope"], "upstream");
    assert_eq!(value["kind"], "net");
    assert_eq!(value["message"], "connection refused");
    assert_eq!(value["code"], 502);

    let deserialized: Error = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.scope(), "upstream");
    assert_eq!(deserialized.kind(), "net");
    assert_eq!(deserialized.message(), "connection refused");
    assert_eq!(deserialized.code(), 502);
}

#[test]
fn round_trip_cfg_variant() {
    let err = Error::cfg("startup", "missing key");
    let json = serde_json::to_string(&err).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["scope"], "startup");
    assert_eq!(value["kind"], "cfg");
    assert_eq!(value["message"], "missing key");
    assert_eq!(value["code"], 500);

    let deserialized: Error = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.scope(), "startup");
    assert_eq!(deserialized.kind(), "cfg");
    assert_eq!(deserialized.message(), "missing key");
    assert_eq!(deserialized.code(), 500);
}

#[test]
fn round_trip_gone_variant() {
    let err = Error::gone("db", "record deleted");
    let json = serde_json::to_string(&err).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["scope"], "db");
    assert_eq!(value["kind"], "gone");
    assert_eq!(value["message"], "record deleted");
    assert_eq!(value["code"], 404);

    let deserialized: Error = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.scope(), "db");
    assert_eq!(deserialized.kind(), "gone");
    assert_eq!(deserialized.message(), "record deleted");
    assert_eq!(deserialized.code(), 404);
}

#[test]
fn deserialize_same_scope_many_times_does_not_leak_unboundedly() {
    let json = r#"{"scope":"parse","kind":"bad","message":"oops","code":400}"#;
    for _ in 0..10_000 {
        let e: Error = serde_json::from_str(json).unwrap();
        assert_eq!(e.scope(), "parse");
    }
}

#[test]
fn deserialize_with_arbitrary_scope_preserves_string() {
    let json = r#"{"scope":"my-custom-scope","kind":"bad","message":"x","code":400}"#;
    let e: Error = serde_json::from_str(json).unwrap();
    assert_eq!(e.scope(), "my-custom-scope");
}
