use mini_search::NumKey;

fn key(v: f64) -> NumKey {
    NumKey::new(v).unwrap()
}

#[test]
fn sort_matches_numeric_order() {
    let mut v = vec![key(2.0), key(-3.5), key(0.0), key(100.0), key(-1.0), key(0.5)];
    v.sort();
    assert_eq!(
        v,
        vec![key(-3.5), key(-1.0), key(0.0), key(0.5), key(2.0), key(100.0)]
    );
}

#[test]
fn negative_zero_equals_zero() {
    assert_eq!(NumKey::new(-0.0).unwrap(), NumKey::new(0.0).unwrap());
}

#[test]
fn nan_returns_invalid_value_error() {
    let result = NumKey::new(f64::NAN);
    assert!(matches!(result, Err(mini_search::Error::InvalidValue { .. })));
}

#[test]
fn from_i64_converts_correctly() {
    let key: NumKey = 42_i64.into();
    assert_eq!(key, NumKey::new(42.0).unwrap());
}

#[test]
fn negative_numbers_sort_correctly() {
    let mut v = vec![key(-1.0), key(-10.0), key(-5.0), key(0.0)];
    v.sort();
    assert_eq!(v, vec![key(-10.0), key(-5.0), key(-1.0), key(0.0)]);
}

#[test]
fn identity_holds() {
    let a = key(3.14);
    assert_eq!(a, a);
}

#[test]
fn ord_is_total() {
    let a = key(1.0);
    let b = key(2.0);
    let c = key(1.0);
    assert!(a < b);
    assert!(b > a);
    assert!(a <= c);
    assert!(a >= c);
}
