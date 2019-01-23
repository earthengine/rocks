use crate::socks5::Socks5Error;
use trust_dns_resolver::AsyncResolver;
use crate::req_addr::ReqAddr;
use crate::error::{Error, MyResultExt};
use crate::connection::{Connection, WrapFramed};

use tokio::net::TcpStream;
use std::net::SocketAddr;
use std::io::ErrorKind;
use tokio::prelude::{Sink,Stream};
use websocket::message::OwnedMessage;
use websocket::ClientBuilder;
use crate::wsocket::WebSocketRequest;

pub struct Socks5Resolver(AsyncResolver);

impl Socks5Resolver {
    pub fn new(resolver: AsyncResolver) -> Self {
        Self(resolver)
    }
    pub async fn connect_websocket(&self, addr:ReqAddr, user: uuid::Uuid) 
        -> Result<impl Connection, (Socks5Error,Error)>
    {
        let (item, dup) = await!((async || -> Result<_,failure::Error> {
            let s = ClientBuilder::new("ws://127.0.0.1:8444")?;
            let (dup,_) = await!(s.add_protocol("rust-websocket")
                                .async_connect_secure(None))?;
            let req = WebSocketRequest::new(addr, user);
            let dup = await!(dup.send(bincode::serialize(&req)?.into()))
                            .map_err(|e| {info!("{}", e); e})?;
            await!(dup.into_future()).map_err(|(e,_)| e.into())
        })())        
        .map_err(|e| (Socks5Error::GeneralProxyFailure, Error::prefix("connect websocket", e)))?;
        info!("{:?}", item);

        if let Some(OwnedMessage::Binary(code))=item {
            let err = code[0].into();
            if err == Socks5Error::Success {
                Ok(WrapFramed::new(dup))        
            } else if err!=Socks5Error::Unknown {
                Err((err , Error::new("error connecting remote")))
            } else {
                Err((Socks5Error::GeneralProxyFailure, Error::new("Invalid reply err code")))
            }
        } else {
            Err((Socks5Error::GeneralProxyFailure , Error::new("invalid reply message")))
        }
    }
    pub async fn connect_tcpstream(&self, addr:ReqAddr) -> Result<TcpStream, (Socks5Error,Error)> {
        let addr = match addr {
            ReqAddr::IP(addr) => addr,
            ReqAddr::Domain(hostname, port) => {
                let lip = await!(self.0.lookup_ip(hostname.as_str()))
                        .map_my_err("lookup_ip")
                        .map_err(|e| (Socks5Error::GeneralProxyFailure, e))?;
                let addr = lip.iter().next().ok_or_else(
                    || (Socks5Error::ConnectionNotAllowed, Error::new("No address")))?;
                info!("{} resolved to {}", hostname, addr);
                SocketAddr::new(addr, port)
            }
        };        
        await!(TcpStream::connect(&addr))
            .map_err(|e| {
                macro_rules! my_err { ($e:ident) => { Error::prefix("connect", $e) } }
                if e.kind() == ErrorKind::ConnectionRefused {
                    (Socks5Error::ConnectionRefused, my_err!(e))
                } else if cfg!(target_os = "windows") {
                    if let Some(1231) = e.raw_os_error() { (Socks5Error::NetworkUnreachable, my_err!(e)) }
                    else if let Some(1232) = e.raw_os_error() { (Socks5Error::HostUnreachable, my_err!(e)) }
                    else if let Some(10060) = e.raw_os_error() { (Socks5Error::TTLExpired, my_err!(e)) }
                    else { (Socks5Error::GeneralProxyFailure,my_err!(e)) }
                } else if cfg!(target_os = "linux") {
                    if let Some(101) = e.raw_os_error() { (Socks5Error::NetworkUnreachable, my_err!(e)) }
                    else if let Some(110) = e.raw_os_error() { (Socks5Error::TTLExpired, my_err!(e)) }
                    else if let Some(113) = e.raw_os_error() { (Socks5Error::HostUnreachable, my_err!(e)) }
                    else { (Socks5Error::GeneralProxyFailure, my_err!(e)) }
                } else {
                    (Socks5Error::GeneralProxyFailure, my_err!(e))
                }
            })
    }
}