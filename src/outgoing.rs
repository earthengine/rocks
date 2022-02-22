use crate::connection::Connection;
use log::error;
use std::io::ErrorKind;
use std::{future::Future, pin::Pin};
use tokio::net::TcpStream;

use crate::config::{OutgoingConfig, OutgoingType};
// use crate::connection::Connection;
use crate::error::Error;
use crate::req_addr::ReqAddr;
// use core::future::Future;
// use failure::{Backtrace, Context, Error, Fail};
// use std::io::ErrorKind;
// use std::pin::Pin;
// use tokio::net::TcpStream;
// //use tokio::prelude::future::Future as TokioFuture;
// use trust_dns_resolver::AsyncResolver;

#[derive(derive_more::Display, Debug)]
pub enum OutgoingError {
    #[display(fmt = "GeneralFailure {}", _0)]
    GeneralFailure(Error),
    //#[display(fmt = "ConnectionNotAllowed {}", _0)]
    //ConnectionNotAllowed(Error),
    #[display(fmt = "NetworkUnreachable {}", _0)]
    NetworkUnreachable(Error),
    #[display(fmt = "HostUnreachable {}", _0)]
    HostUnreachable(Error),
    #[display(fmt = "ConnectionRefused {}", _0)]
    ConnectionRefused(Error),
    #[display(fmt = "Timed out {}", _0)]
    TimedOut(Error),
    //#[display(fmt = "Unknown {}", _0)]
    //Unknown(Error),
}
// #[allow(dead_code)]
impl OutgoingError {
    fn general(e: Error) -> Self {
        OutgoingError::GeneralFailure(e)
    }
    //     fn connection_not_allowed(e: Error) -> Self {
    //         OutgoingError::ConnectionNotAllowed(Backtrace::new(), e)
    //     }
    fn network_unreachable(e: Error) -> Self {
        OutgoingError::NetworkUnreachable(e)
    }
    fn host_unreachable(e: Error) -> Self {
        OutgoingError::HostUnreachable(e)
    }
    fn connection_refused(e: Error) -> Self {
        OutgoingError::ConnectionRefused(e)
    }
    fn timed_out(e: Error) -> Self {
        OutgoingError::TimedOut(e)
    }
    //     fn unknown(e: Error) -> Self {
    //         OutgoingError::Unknown(Backtrace::new(), e)
    //     }
}

pub trait Outgoing: Clone {
    type Stream: Connection + Send;
    fn process_request(
        self,
        req: ReqAddr,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Stream, OutgoingError>> + Send>>;
}

#[derive(Clone, Debug)]
struct DirectOutgoing;
impl Outgoing for DirectOutgoing {
    type Stream = TcpStream;
    fn process_request(
        self,
        req: ReqAddr,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Stream, OutgoingError>> + Send>> {
        Box::pin(self.process_request_impl(req))
    }
}

#[cfg(target_os = "windows")]
mod network_errors {
    pub const NETWORK_UNREACHABLE: i32 = 1231;
    pub const HOST_UNREACHABLE: i32 = 1232;
    pub const TTL_EXPIRED: i32 = 10060;
}
#[cfg(target_os = "linux")]
mod network_errors {
    pub const NETWORK_UNREACHABLE: i32 = 101;
    pub const HOST_UNREACHABLE: i32 = 110;
    pub const TTL_EXPIRED: i32 = 113;
}
#[cfg(target_os = "macos")]
mod network_errors {
    pub const NETWORK_UNREACHABLE: i32 = 2;
    pub const HOST_UNREACHABLE: i32 = 1;
    pub const TTL_EXPIRED: i32 = -72006;
}

impl DirectOutgoing {
    async fn process_request_impl(self, req: ReqAddr) -> Result<TcpStream, OutgoingError> {
        let addr = req.resolve_local().map_err(|e| {
            error!("{} {}", req, e);
            OutgoingError::HostUnreachable(e)
        })?;
        Ok(TcpStream::connect(&addr).await.map_err(|e| {
            if e.kind() == ErrorKind::ConnectionRefused {
                OutgoingError::connection_refused(e.into())
            } else {
                match e.raw_os_error() {
                    Some(network_errors::NETWORK_UNREACHABLE) => {
                        OutgoingError::network_unreachable(e.into())
                    }
                    Some(network_errors::HOST_UNREACHABLE) => {
                        OutgoingError::host_unreachable(e.into())
                    }
                    Some(network_errors::TTL_EXPIRED) => OutgoingError::timed_out(e.into()),
                    _ => OutgoingError::general(e.into()),
                }
            }
        })?)
    }
}

pub fn get_outgoing<'a>(conf: OutgoingConfig) -> Result<impl Outgoing, Error> {
    match conf.r#type {
        OutgoingType::Direct => Ok(DirectOutgoing),
        _ => Err(Error::from_description("Unsupported outgoing type")),
    }
}
