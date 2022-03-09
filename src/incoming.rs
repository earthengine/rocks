use crate::config::{IncomingConfig, IncomingType};
use crate::connection::Connection;
use crate::error::Error;
use crate::outgoing::OutgoingError;
use crate::req_addr::ReqAddr;
use crate::socks5::Socks5Incoming;
use crate::StandardFuture;

pub trait Incoming {
    type Client: IncomingClient + Send;
    fn next_client<'a>(&'a mut self) -> StandardFuture<'a, Self::Client, Error>;
}
pub trait IncomingClient {
    type Connection: Connection + Send;
    fn next_request<'a>(&'a mut self) -> StandardFuture<'a, ReqAddr, Error>;
    fn abort(self, err: OutgoingError, req: ReqAddr) -> StandardFuture<'static, (), Error>;
    fn ready_for_connect<'a>(
        &'a mut self,
        req: ReqAddr,
    ) -> StandardFuture<'a, Self::Connection, Error>;
}

pub async fn get_incoming<'a>(conf: IncomingConfig) -> Result<impl Incoming, Error> {
    match conf.r#type {
        IncomingType::Socks5 => Socks5Incoming::from_cfg(conf.listen_addr).await,
        _ => Err(Error::from_description("Unsupported incoming type")),
    }
}
