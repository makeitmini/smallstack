use std::collections::HashMap;

use hyper::Request;

pub fn get_header<'a, B>(req: &'a Request<B>, name: &str) -> Option<&'a str> {
    req.headers().get(name)?.to_str().ok()
}

pub fn parse_cookies<B>(req: &Request<B>) -> HashMap<String, String> {
    let mut cookies = HashMap::new();
    if let Some(cookie_header) = req.headers().get("cookie") {
        if let Ok(value) = cookie_header.to_str() {
            for pair in value.split(';').filter(|s| !s.is_empty()) {
                let pair = pair.trim();
                if let Some((key, value)) = pair.split_once('=') {
                    cookies.insert(key.trim().to_string(), value.trim().to_string());
                }
            }
        }
    }
    cookies
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::body::Bytes;
    use http_body_util::Full;

    fn builder() -> hyper::http::request::Builder {
        hyper::Request::builder()
    }

    #[test]
    fn get_header_returns_value_for_existing_header() {
        let req = builder()
            .header("x-foo", "bar")
            .body(Full::new(Bytes::new()))
            .unwrap();
        assert_eq!(get_header(&req, "x-foo"), Some("bar"));
    }

    #[test]
    fn get_header_returns_none_for_missing_header() {
        let req = builder()
            .body(Full::new(Bytes::new()))
            .unwrap();
        assert_eq!(get_header(&req, "x-missing"), None);
    }

    #[test]
    fn get_header_is_case_sensitive() {
        let req = builder()
            .header("X-Foo", "bar")
            .body(Full::new(Bytes::new()))
            .unwrap();
        assert_eq!(get_header(&req, "X-Foo"), Some("bar"));
        assert_eq!(get_header(&req, "x-foo"), Some("bar"));
        assert_eq!(get_header(&req, "X-FOO"), Some("bar"));
    }

    #[test]
    fn parse_cookies_returns_empty_map_for_no_cookie_header() {
        let req = builder()
            .body(Full::new(Bytes::new()))
            .unwrap();
        assert!(parse_cookies(&req).is_empty());
    }

    #[test]
    fn parse_cookies_extracts_single_cookie() {
        let req = builder()
            .header("cookie", "session=abc123")
            .body(Full::new(Bytes::new()))
            .unwrap();
        let cookies = parse_cookies(&req);
        assert_eq!(cookies.get("session").unwrap(), "abc123");
    }

    #[test]
    fn parse_cookies_extracts_multiple_cookies() {
        let req = builder()
            .header("cookie", "session=abc123; theme=dark")
            .body(Full::new(Bytes::new()))
            .unwrap();
        let cookies = parse_cookies(&req);
        assert_eq!(cookies.get("session").unwrap(), "abc123");
        assert_eq!(cookies.get("theme").unwrap(), "dark");
    }

    #[test]
    fn parse_cookies_handles_whitespace() {
        let req = builder()
            .header("cookie", " session = abc123 ; theme = dark ")
            .body(Full::new(Bytes::new()))
            .unwrap();
        let cookies = parse_cookies(&req);
        assert_eq!(cookies.get("session").unwrap(), "abc123");
        assert_eq!(cookies.get("theme").unwrap(), "dark");
    }
}
