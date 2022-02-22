use crate::config::{IncomingConfig, IncomingType};
use crate::error::Error;
use crate::outgoing::Outgoing;
use crate::socks5::Socks5Incoming;
use std::future::Future;
use std::pin::Pin;

pub trait Incoming: Clone {
    fn start<O>(self, outgoing: O) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>
    where
        O: Outgoing + Send + Unpin + 'static;
}

pub fn get_incoming<'a>(conf: IncomingConfig) -> Result<impl Incoming, Error> {
    match conf.r#type {
        IncomingType::Socks5 => Socks5Incoming::from_cfg(conf.listen_addr),
        _ => Err(Error::from_description("Unsupported incoming type")),
    }
}
