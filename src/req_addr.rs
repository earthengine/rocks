use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{LocalWaker, Poll};
use crate::connection::Connection;
use crate::error::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ReqAddr {
    IP(SocketAddr),
    Domain(String, u16)
}
impl ReqAddr {
    pub fn from_addr(addr: SocketAddr) -> Self {
        ReqAddr::IP(addr)
    }
    pub fn from_domain(domain: impl Into<String>, port: u16) -> Self {
        ReqAddr::Domain(domain.into(), port)
    }
    pub fn port(&self) -> u16 {
        match self {
            ReqAddr::IP(addr) => addr.port(),
            ReqAddr::Domain(_,port) => *port
        }
    }
}
impl Default for ReqAddr {
    fn default() -> Self {
        ReqAddr::IP("127.0.0.1:0".parse().unwrap())
    }
}

pub trait Resolver {
    type Resolved: Connection;
    fn poll_connect(self: Pin<&mut Self>, addr:ReqAddr, lw: &LocalWaker) -> Poll<Result<Self::Resolved, Error>>;
}