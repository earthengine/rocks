use crate::outgoing::Outgoing;
use core::{future::Future,pin::Pin};
use crate::config::{IncomingConfig, IncomingType};
use crate::socks5::Socks5Incoming;
use failure::Error;

pub trait Incoming: Clone
{
    fn start<O>(self, outgoing: O) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>
    where
        O: Outgoing + Send + Unpin + 'static;
}

pub fn get_incoming(conf: IncomingConfig) -> Result<impl Incoming, Error> {
    match conf.r#type {
        IncomingType::Socks5 => Socks5Incoming::from_cfg(conf.listen_addr),
        _ => bail!("Unsupported incoming type"),
    }
}
