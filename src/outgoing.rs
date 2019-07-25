use crate::config::{OutgoingConfig, OutgoingType};
use crate::connection::Connection;
use crate::req_addr::ReqAddr;
use core::future::Future;
use failure::{Backtrace, Context, Error, Fail};
use std::io::ErrorKind;
use std::pin::Pin;
use tokio::net::TcpStream;
//use tokio::prelude::future::Future as TokioFuture;
use trust_dns_resolver::AsyncResolver;

#[derive(Fail, Debug)]
pub enum OutgoingError {
    #[fail(display = "GeneralFailure {}", _1)]
    GeneralFailure(Backtrace, #[fail(cause)] Error),
    #[fail(display = "ConnectionNotAllowed {}", _1)]
    ConnectionNotAllowed(Backtrace, #[fail(cause)] Error),
    #[fail(display = "NetworkUnreachable {}", _1)]
    NetworkUnreachable(Backtrace, #[fail(cause)] Error),
    #[fail(display = "HostUnreachable {}", _1)]
    HostUnreachable(Backtrace, #[fail(cause)] Error),
    #[fail(display = "ConnectionRefused {}", _1)]
    ConnectionRefused(Backtrace, #[fail(cause)] Error),
    #[fail(display = "Timed out {}", _1)]
    TimedOut(Backtrace, #[fail(cause)] Error),
    #[fail(display = "Unknown {}", _1)]
    Unknown(Backtrace, #[fail(cause)] Error),
}
#[allow(dead_code)]
impl OutgoingError {
    fn general(e: Error) -> Self {
        OutgoingError::GeneralFailure(Backtrace::new(), e)
    }
    fn connection_not_allowed(e: Error) -> Self {
        OutgoingError::ConnectionNotAllowed(Backtrace::new(), e)
    }
    fn network_unreachable(e: Error) -> Self {
        OutgoingError::NetworkUnreachable(Backtrace::new(), e)
    }
    fn host_unreachable(e: Error) -> Self {
        OutgoingError::HostUnreachable(Backtrace::new(), e)
    }
    fn connection_refused(e: Error) -> Self {
        OutgoingError::ConnectionRefused(Backtrace::new(), e)
    }
    fn timed_out(e: Error) -> Self {
        OutgoingError::TimedOut(Backtrace::new(), e)
    }
    fn unknown(e: Error) -> Self {
        OutgoingError::Unknown(Backtrace::new(), e)
    }
}

pub trait Outgoing: Clone {
    type Stream: Connection + Send;
    fn process_request(
        self,
        req: ReqAddr,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Stream, OutgoingError>> + Send>>;
}

#[derive(Clone, Debug)]
struct DirectOutgoing(AsyncResolver);
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
            OutgoingError::general(e)
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

pub fn get_outgoing(conf: OutgoingConfig, resolver: AsyncResolver) -> Result<impl Outgoing, Error> {
    match conf.r#type {
        OutgoingType::Direct => Ok(DirectOutgoing(resolver)),
        _ => Err(Context::new("Unsupported outgoing type").into()),
    }
}
