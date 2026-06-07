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

