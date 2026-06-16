use mini_search::Tokenizer;

#[test]
fn basic_punctuation_and_case() {
    let t = Tokenizer::new();
    assert_eq!(
        t.tokenize("NSAID, for Dogs!").unwrap(),
        vec!["nsaid", "for", "dogs"]
    );
}

#[test]
fn min_token_len_one_keeps_single_chars() {
    let t = Tokenizer::new();
    assert_eq!(t.tokenize("vitamin c").unwrap(), vec!["vitamin", "c"]);
}

#[test]
fn unterminated_quote_tokenises_remainder() {
    let t = Tokenizer::new();
    assert_eq!(
        t.tokenize("\"dog cat").unwrap(),
        vec!["dog cat"]
    );
}

#[test]
fn matched_quote_is_phrase_token() {
    let t = Tokenizer::new();
    assert_eq!(
        t.tokenize("\"dog cat\" food").unwrap(),
        vec!["dog cat", "food"]
    );
}

#[test]
fn unicode_lowercasing() {
    let t = Tokenizer::new();
    assert_eq!(t.tokenize("Müller").unwrap(), vec!["müller"]);
}

#[test]
fn apostrophe_is_kept() {
    let t = Tokenizer::new();
    assert_eq!(t.tokenize("don't stop").unwrap(), vec!["don't", "stop"]);
}

#[test]
fn input_over_max_bytes_returns_error() {
    let t = Tokenizer::new();
    let big = "a".repeat(1025);
    let result = t.tokenize(&big);
    assert!(matches!(result, Err(mini_search::Error::InvalidQuery { .. })));
}

#[test]
fn empty_string_returns_empty() {
    let t = Tokenizer::new();
    let result = t.tokenize("").unwrap();
    assert!(result.is_empty());
}

#[test]
fn whitespace_only_returns_empty() {
    let t = Tokenizer::new();
    let result = t.tokenize("   \t\n  ").unwrap();
    assert!(result.is_empty());
}

#[test]
fn order_is_preserved() {
    let t = Tokenizer::new();
    assert_eq!(
        t.tokenize("dog cat bird").unwrap(),
        vec!["dog", "cat", "bird"]
    );
}

#[test]
fn punctuation_is_stripped() {
    let t = Tokenizer::new();
    assert_eq!(
        t.tokenize("hello... world!!! test???").unwrap(),
        vec!["hello", "world", "test"]
    );
}

#[test]
fn numbers_are_kept_as_tokens() {
    let t = Tokenizer::new();
    assert_eq!(t.tokenize("abc 123 xyz").unwrap(), vec!["abc", "123", "xyz"]);
}

#[test]
fn leading_trailing_whitespace_is_handled() {
    let t = Tokenizer::new();
    assert_eq!(
        t.tokenize("  hello world  ").unwrap(),
        vec!["hello", "world"]
    );
}
