use std::time::SystemTime;

use serde_json::{Map, Value};

use crate::{Entry, Level};

fn unix_ts() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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

        let mut out = format!("[{}] {}({}): {}", unix_ts(), level, self.logger.scope, self.msg);
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

        let mut map = Map::new();
        map.insert("level".into(), Value::String(level.into()));
        map.insert("scope".into(), Value::String(self.logger.scope.into()));
        map.insert("msg".into(), Value::String(self.msg.into()));

        for i in 0..self.count {
            if let Some((key, ref val)) = self.fields[i] {
                map.insert(key.to_string(), Value::String(val.clone()));
            }
        }

        map.insert("ts".into(), Value::Number(unix_ts().into()));

        serde_json::to_string(&Value::Object(map)).unwrap_or_default()
    }
}
