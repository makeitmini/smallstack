pub trait Transform: Send + Sync + 'static {
    fn apply(&self, content_type: &str, body: Vec<u8>) -> Vec<u8>;
}

impl<F> Transform for F
where
    F: Fn(&str, Vec<u8>) -> Vec<u8> + Send + Sync + 'static,
{
    fn apply(&self, content_type: &str, body: Vec<u8>) -> Vec<u8> {
        (self)(content_type, body)
    }
}
