use std::time::SystemTime;

use crate::{Entry, Level};

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

impl Entry<'_> {
    pub fn render(&self) -> String {
        let level = match self.level {
            Level::Error => "error",
            Level::Warn  => "warn",
            Level::Info  => "info",
            Level::Debug => "debug",
            Level::Trace => "trace",
        };

        let mut out = format!("{}({}): {}", level, self.logger.scope, self.msg);
        for i in 0..self.count {
            if let Some((key, ref val)) = self.fields[i] {
                out.push_str(&format!(" {key}={val}"));
            }
        }
        out
    }

    pub fn render_json(&self) -> String {
        let level = match self.level {
            Level::Error => "error",
            Level::Warn  => "warn",
            Level::Info  => "info",
            Level::Debug => "debug",
            Level::Trace => "trace",
        };

        let mut s = String::new();
        s.push_str(&format!(
            r#"{{"level":"{}","scope":"{}","msg":"{}""#,
            level,
            escape_json(self.logger.scope),
            escape_json(self.msg),
        ));

        for i in 0..self.count {
            if let Some((key, ref val)) = self.fields[i] {
                s.push_str(&format!(",\"{}\":\"{}\"", escape_json(key), escape_json(val)));
            }
        }

        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        s.push_str(&format!(",\"ts\":{ts}}}"));
        s
    }
}
