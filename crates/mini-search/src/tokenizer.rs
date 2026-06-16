use crate::bounds::MAX_QUERY_BYTES;
use crate::error::{Error, Result};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Tokenizer {
    min_token_len: usize,
    stopwords: HashSet<String>,
}

impl Tokenizer {
    pub fn new() -> Self {
        Tokenizer {
            min_token_len: 1,
            stopwords: HashSet::new(),
        }
    }

    pub fn with_min_token_len(mut self, len: usize) -> Self {
        self.min_token_len = len;
        self
    }

    pub fn with_stopwords(mut self, words: HashSet<String>) -> Self {
        self.stopwords = words;
        self
    }

    pub fn tokenize(&self, text: &str) -> Result<Vec<String>> {
        if text.len() > MAX_QUERY_BYTES {
            return Err(Error::invalid_query(
                "input exceeds maximum query length",
            ));
        }

        let mut tokens = Vec::new();
        let mut chars = text.chars().peekable();

        while let Some(&ch) = chars.peek() {
            if ch.is_whitespace() {
                chars.next();
                continue;
            }

            if ch == '"' {
                chars.next();
                let mut phrase = String::new();
                loop {
                    match chars.next() {
                        None => break,
                        Some('"') => break,
                        Some(c) => phrase.extend(c.to_lowercase()),
                    }
                }
                if !phrase.is_empty() {
                    tokens.push(phrase);
                }
                continue;
            }

            let mut word = String::new();
            loop {
                match chars.peek() {
                    None => break,
                    Some(&c) if c.is_whitespace() || c == '"' => break,
                    Some(&c) => {
                        chars.next();
                        if c.is_alphanumeric() || c == '\'' {
                            word.extend(c.to_lowercase());
                        }
                    }
                }
            }

            if word.len() >= self.min_token_len && !self.stopwords.contains(&word) {
                tokens.push(word);
            }
        }

        Ok(tokens)
    }
}

impl Default for Tokenizer {
    fn default() -> Self {
        Self::new()
    }
}
