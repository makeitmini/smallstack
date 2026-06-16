use crate::bounds::{MAX_QUERY_BYTES, MAX_QUERY_TERMS};
use crate::error::{Error, Result};
use crate::index::Comparison;

#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub text: Vec<TextClause>,
    pub filters: Vec<Filter>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextClause {
    pub field: Option<String>,
    pub term: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    Compare { field: String, op: Comparison, value: f64 },
    Range { field: String, low: f64, high: f64 },
    Exact { field: String, value: String },
}

impl Filter {
    pub fn cmp(field: &str, op: Comparison, value: f64) -> Self {
        Filter::Compare {
            field: field.to_string(),
            op,
            value,
        }
    }

    pub fn range(field: &str, low: f64, high: f64) -> Self {
        Filter::Range {
            field: field.to_string(),
            low,
            high,
        }
    }

    pub fn eq(field: &str, value: &str) -> Self {
        Filter::Exact {
            field: field.to_string(),
            value: value.to_string(),
        }
    }
}

impl Query {
    pub fn text(term: &str) -> Self {
        Query {
            text: vec![TextClause {
                field: None,
                term: term.to_string(),
            }],
            filters: vec![],
        }
    }

    pub fn fielded(field: &str, term: &str) -> Self {
        Query {
            text: vec![TextClause {
                field: Some(field.to_string()),
                term: term.to_string(),
            }],
            filters: vec![],
        }
    }

    pub fn parse(input: &str) -> Result<Self> {
        if input.len() > MAX_QUERY_BYTES {
            return Err(Error::invalid_query(
                "input exceeds maximum query length",
            ));
        }

        let mut text = Vec::new();
        let mut filters = Vec::new();
        let mut term_count = 0;

        for token in split_query_tokens(input) {
            term_count += 1;
            if term_count > MAX_QUERY_TERMS {
                return Err(Error::invalid_query("too many query terms"));
            }

            if token.starts_with('"') {
                let content = if token.ends_with('"') && token.len() > 1 {
                    &token[1..token.len() - 1]
                } else {
                    &token[1..]
                };
                if !content.is_empty() {
                    text.push(TextClause {
                        field: None,
                        term: content.to_lowercase(),
                    });
                }
                continue;
            }

            if let Some(pos) = token.find(':') {
                let field = &token[..pos];
                let value = &token[pos + 1..];

                if value.is_empty() {
                    return Err(Error::invalid_query(format!(
                        "empty value for field '{}'",
                        field
                    )));
                }

                let first = value.chars().next().unwrap();
                if first == '>' || first == '<' || first == '=' {
                    let (op, raw) = split_operator(value);
                    if op == "=" {
                        if let Ok(num) = raw.parse::<f64>() {
                            filters.push(Filter::Compare {
                                field: field.to_string(),
                                op: Comparison::Eq,
                                value: num,
                            });
                        } else {
                            filters.push(Filter::Exact {
                                field: field.to_string(),
                                value: raw.to_string(),
                            });
                        }
                    } else {
                        let num: f64 = raw
                            .parse()
                            .map_err(|_| Error::invalid_query("invalid number in comparison"))?;
                        let op = match op {
                            ">=" => Comparison::Gte,
                            ">" => Comparison::Gt,
                            "<=" => Comparison::Lte,
                            "<" => Comparison::Lt,
                            _ => return Err(Error::invalid_query("unknown operator")),
                        };
                        filters.push(Filter::Compare {
                            field: field.to_string(),
                            op,
                            value: num,
                        });
                    }
                } else if value.starts_with('[') {
                    let inner = value
                        .strip_prefix('[')
                        .and_then(|s| s.strip_suffix(']'))
                        .unwrap_or("");
                    let parts: Vec<&str> = inner.splitn(2, "TO").map(|s| s.trim()).collect();
                    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
                        return Err(Error::invalid_query("invalid range syntax"));
                    }
                    let low: f64 = parts[0]
                        .parse()
                        .map_err(|_| Error::invalid_query("invalid range bound"))?;
                    let high: f64 = parts[1]
                        .parse()
                        .map_err(|_| Error::invalid_query("invalid range bound"))?;
                    filters.push(Filter::Range {
                        field: field.to_string(),
                        low,
                        high,
                    });
                } else if value == "true" || value == "false" {
                    filters.push(Filter::Exact {
                        field: field.to_string(),
                        value: value.to_string(),
                    });
                } else {
                    text.push(TextClause {
                        field: Some(field.to_string()),
                        term: value.to_lowercase(),
                    });
                }
            } else {
                text.push(TextClause {
                    field: None,
                    term: token.to_lowercase(),
                });
            }
        }

        Ok(Query { text, filters })
    }
}

fn split_query_tokens(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        if ch == '"' {
            let mut token = String::new();
            token.push('"');
            chars.next();
            loop {
                match chars.next() {
                    None => break,
                    Some('"') => {
                        token.push('"');
                        break;
                    }
                    Some(c) => token.push(c),
                }
            }
            tokens.push(token);
        } else {
            let mut token = String::new();
            let mut bracket_depth: usize = 0;
            while let Some(&ch) = chars.peek() {
                if ch == '[' {
                    bracket_depth += 1;
                } else if ch == ']' {
                    bracket_depth = bracket_depth.saturating_sub(1);
                }
                if ch.is_whitespace() && bracket_depth == 0 {
                    break;
                }
                token.push(ch);
                chars.next();
            }
            tokens.push(token);
        }
    }

    tokens
}

fn split_operator(value: &str) -> (&str, &str) {
    if value.starts_with(">=") || value.starts_with("<=") {
        (&value[..2], &value[2..])
    } else if value.starts_with('>') || value.starts_with('<') || value.starts_with('=') {
        (&value[..1], &value[1..])
    } else {
        ("", value)
    }
}
