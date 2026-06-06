use crate::{Entry, Level};

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
}
