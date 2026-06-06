use std::fmt::Display;

use crate::Level;

pub struct Entry<'a> {
    pub logger: &'a crate::Logger,
    pub level:  Level,
    pub msg:    &'static str,
    pub fields: [Option<(&'static str, String)>; 8],
    pub count:  usize,
}

impl<'a> Entry<'a> {
    pub fn field(mut self, key: &'static str, val: impl Display) -> Self {
        if self.count < 8 {
            self.fields[self.count] = Some((key, val.to_string()));
            self.count += 1;
        } else {
            #[cfg(debug_assertions)]
            panic!("entry field overflow: max 8 fields");
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
