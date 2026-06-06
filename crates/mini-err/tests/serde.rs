#![cfg(feature = "serde")]

use mini_err::Error;

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
