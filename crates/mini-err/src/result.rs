use crate::Error;

pub type Result<T> = std::result::Result<T, Error>;

pub trait ErrorExt<T> {
    fn context(self, scope: &'static str, msg: impl Into<String>) -> Result<T>;
}

impl<T, E: Into<Error>> ErrorExt<T> for std::result::Result<T, E> {
    fn context(self, scope: &'static str, msg: impl Into<String>) -> Result<T> {
        self.map_err(|e| {
            let mut err: Error = e.into();
            match &mut err {
                Error::Io { scope: s, msg: m, .. } => {
                    *s = scope;
                    *m = Some(msg.into());
                }
                Error::Net { msg: m, scope: s }
                | Error::Cfg { msg: m, scope: s }
                | Error::Bad { msg: m, scope: s }
                | Error::Gone { msg: m, scope: s } => {
                    *s = scope;
                    *m = msg.into();
                }
            }
            err
        })
    }
}
