use std::fmt::Display;

use crate::Level;

/// Replace characters that could be used for log injection or terminal
/// escape injection with a safe substitute.
///
/// Control characters below `0x20` (except `\t`) and DEL (`0x7f`) are
/// unsafe in terminal output; `\r`, `\n`, `\0` are the classic log-injection
/// set. The fast path avoids any allocation when the input is clean.
fn sanitize(val: &str) -> String {
    let needs_sanitize = val.contains(|c: char| {
        c == '\r' || c == '\n' || c == '\0'
            || (c < '\x20' && c != '\t')
            || c == '\x7f'
    });
    if !needs_sanitize {
        return val.to_owned();
    }
    val.chars()
        .map(|c| {
            if c == '\t' {
                c
            } else if c < '\x20' || c == '\x7f' {
                ' '
            } else {
                c
            }
        })
        .collect()
}

pub struct Entry<'a> {
    pub logger: &'a crate::Logger,
    pub level:  Level,
    pub msg:    &'static str,
    pub fields: [Option<(&'static str, String)>; 8],
    pub count:  usize,
    pub overflow_count: usize,
}

impl<'a> Entry<'a> {
    #[cfg(feature = "err")]
    pub fn err(self, error: &mini_err::Error) -> Self {
        self.field("err_scope", error.scope())
            .field("err_kind", error.kind())
            .field("err_msg", error.message())
            .field("err_code", error.code().to_string())
    }

    pub fn duration(self, start: std::time::Instant) -> Self {
        let ms = start.elapsed().as_millis();
        self.field("duration", format!("{ms}ms"))
    }

    pub fn field(mut self, key: &'static str, val: impl Display) -> Self {
        if self.count < 8 {
            self.fields[self.count] = Some((key, sanitize(&val.to_string())));
            self.count += 1;
        } else {
            self.overflow_count += 1;
            self.fields[7] = Some(("fields_truncated", self.overflow_count.to_string()));
        }
        self
    }

    pub fn emit(self) {
        if self.level > self.logger.level {
            return;
        }

        let rendered = match self.logger.format {
            crate::Format::Conventional => self.render(),
            crate::Format::Json => self.render_json(),
        };
        if let Ok(mut guard) = self.logger.out.lock() {
            use std::io::Write;
            let _ = writeln!(guard, "{rendered}");
        }
    }
}
