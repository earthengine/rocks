use failure::Error;
use std::io::Read;
use tokio::io::AsyncRead;
use tokio::prelude::AsyncReadExt;

pub struct Preread<R> {
    preread: Vec<u8>,
    read: R,
}
impl<R> Preread<R>
where
    R: AsyncRead,
{
    pub fn new(preread: impl Into<Vec<u8>>, r: R) -> Self {
        Self {
            preread: preread.into(),
            read: r,
        }
    }
    pub async fn read(&mut self) -> Result<Vec<u8>, Error> {
        let mut v = [0u8; 4096];
        let c = await!(self.read.read_async(&mut v))?;
        let mut r = std::mem::replace(&mut self.preread, vec![]);
        r.extend(&v[0..c]);
        Ok(r)
    }
}
