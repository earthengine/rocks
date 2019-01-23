#[derive(Debug)]
pub struct Error(String, Option<failure::Error>);
impl Error {
    pub fn new(ctx: impl Into<String>) -> Self {
        Self(ctx.into(), None)
    }
    pub fn prefix(prefix: impl Into<String>, f: impl Into<failure::Error>) -> Self {
        Self(prefix.into(), Some(f.into()))
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{} {}",
            self.0,
            match self.1 {
                Some(ref v) => format!("{}", v),
                None => "no error message".into(),
            }
        )
    }
}
impl std::error::Error for Error {}
pub trait MyResultExt<T, E> {
    fn map_my_err(self, ctx: impl Into<String>) -> Result<T, Error>
    where
        Self: Sized,
        E: Into<failure::Error>;
}
impl<T, E> MyResultExt<T, E> for Result<T, E> {
    fn map_my_err(self, ctx: impl Into<String>) -> Result<T, Error>
    where
        Self: Sized,
        E: Into<failure::Error>,
    {
        self.map_err(|e| {
            let (ctx, e) = (ctx.into(), e.into());
            Error::prefix(ctx, e)
        })
    }
}
