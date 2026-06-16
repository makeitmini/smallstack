use crate::engine::SearchHit;
use crate::error::Result;

pub trait Processor {
    fn pre_search(&self, query: &str) -> Result<String> {
        Ok(query.to_string())
    }

    fn post_search(&self, hits: Vec<SearchHit>) -> Result<Vec<SearchHit>> {
        Ok(hits)
    }
}
