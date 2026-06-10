use std::time::SystemTime;

use serde_json::{Map, Value};

use crate::Entry;

fn unix_ts_millis() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Entry<'_> {
    pub fn render(&self) -> String {
        let level = self.level.as_str();

        let ts = unix_ts_millis();
        let secs = ts / 1000;
        let sub_sec = ts % 1000;
        let mut out = format!("[{secs}.{sub_sec:03}] {level}({}): {}", self.logger.scope, self.msg);
        for i in 0..self.count {
            if let Some((key, ref val)) = self.fields[i] {
                out.push_str(&format!(" {key}={val}"));
            }
        }
        out
    }

    pub fn render_json(&self) -> String {
        let level = self.level.as_str();

        let mut map = Map::new();
        map.insert("level".into(), Value::String(level.into()));
        map.insert("scope".into(), Value::String(self.logger.scope.into()));
        map.insert("msg".into(), Value::String(self.msg.into()));

        for i in 0..self.count {
            if let Some((key, ref val)) = self.fields[i] {
                map.insert(key.to_string(), Value::String(val.clone()));
            }
        }

        map.insert("ts".into(), Value::Number(unix_ts_millis().into()));

        serde_json::to_string(&Value::Object(map)).unwrap_or_default()
    }
}
