use log::{debug, info};
use std::convert::TryFrom;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::incoming::{Incoming, IncomingClient};
use crate::outgoing::OutgoingError;
use crate::socks5::{
    Socks5AddrType, Socks5Error, SOCKS5_CMD_CONNECT, SOCKS5_NO_ACCEPTABLE_METHOD, SOCKS5_NO_AUTH,
    SOCKS5_PROTOCOL,
};
use crate::{config::CfgAddr, error::Error, req_addr::ReqAddr, StandardFuture};

#[derive(Debug)]
pub(crate) struct Socks5Incoming {
    listen_addr: SocketAddr,
    listener: TcpListener,
}

pub struct Socks5Connected {
    _local_addr: SocketAddr,
    _remote_addr: SocketAddr,
    stream: Option<TcpStream>,
}
impl IncomingClient for Socks5Connected {
    type Connection = TcpStream;
    fn next_request<'a>(&'a mut self) -> StandardFuture<'a, ReqAddr, Error> {
        Box::pin(async move {
            self.authenticate_client().await?;
            self.get_request().await
        })
    }
    fn abort(mut self, err: OutgoingError, req: ReqAddr) -> StandardFuture<'static, (), Error> {
        let reason = self.get_reason(&err);
        Box::pin(async move { self.send_final_response(reason, req).await })
    }
    fn ready_for_connect<'a>(
        &'a mut self,
        req: ReqAddr,
    ) -> StandardFuture<'a, Self::Connection, Error> {
        Box::pin(async move {
            self.send_final_response(Socks5Error::Success, req).await?;
            let stream = self.stream.take().unwrap();
            Ok(stream)
        })
    }
}

impl Incoming for Socks5Incoming {
    type Client = Socks5Connected;
    fn next_client<'a>(&'a mut self) -> StandardFuture<'a, Self::Client, Error> {
        Box::pin(self.next_client_impl())
    }
}

impl Socks5Incoming {
    pub async fn from_cfg(conf: CfgAddr) -> Result<Self, Error> {
        let listen_addr = conf.into_addr()?;
        Ok(Socks5Incoming {
            listen_addr,
            listener: TcpListener::bind(listen_addr).await?,
        })
    }
    async fn next_client_impl<'a>(&'a mut self) -> Result<Socks5Connected, Error> {
        let (stream, incoming_addr) = self.listener.accept().await?;
        info!("incoming!");
        let st = Socks5Connected {
            _local_addr: self.listen_addr,
            _remote_addr: incoming_addr,
            stream: Some(stream),
        };
        Ok(st)
    }
}

impl Socks5Connected {
    fn get_reason(&self, err: &OutgoingError) -> Socks5Error {
        match err {
            OutgoingError::GeneralFailure(..) => Socks5Error::GeneralProxyFailure,
            // OutgoingError::ConnectionNotAllowed(..) => Socks5Error::ConnectionNotAllowed,
            OutgoingError::NetworkUnreachable(..) => Socks5Error::NetworkUnreachable,
            OutgoingError::HostUnreachable(..) => Socks5Error::HostUnreachable,
            OutgoingError::ConnectionRefused(..) => Socks5Error::ConnectionRefused,
            OutgoingError::TimedOut(..) => Socks5Error::TTLExpired,
            // OutgoingError::Unknown(..) => Socks5Error::Unknown,
        }
    }

    async fn read_exact<'a>(&'a mut self, buf: &'a mut [u8]) -> Result<usize, Error> {
        match self.stream.as_mut() {
            Some(stream) => Ok(stream.read_exact(buf).await?),
            None => Err(Error::NotConnected),
        }
    }
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        match self.stream.as_mut() {
            Some(stream) => Ok(stream.write_all(buf).await?),
            None => Err(Error::NotConnected),
        }
    }

    async fn authenticate_client(&mut self) -> Result<(), Error> {
        let mut buf = [0u8; 256];
        info!("authenticate client");
        self.read_exact(&mut buf[0..2]).await?;
        let p = buf[0];
        if p != SOCKS5_PROTOCOL {
            Err(Error::from_description(&format!(
                "Not SOCKS5 protocol - {}",
                p
            )))?;
        }

        let num_auth_methods = buf[1];
        self.read_exact(&mut buf[0..num_auth_methods as usize])
            .await?;
        let authenticate_methods = &mut buf[0..num_auth_methods as usize];
        if !authenticate_methods.contains(&SOCKS5_NO_AUTH) {
            self.write_all(&[SOCKS5_PROTOCOL, SOCKS5_NO_ACCEPTABLE_METHOD])
                .await?;
            Err(Error::from_description("No supported method given"))?;
        }
        info!(
            "client authenticate successfully ({} -> {})",
            self._remote_addr, self._local_addr,
        );
        self.write_all(&[SOCKS5_PROTOCOL, SOCKS5_NO_AUTH]).await?;
        info!("wrote auth response");
        Ok(())
    }

    pub async fn get_request(&mut self) -> Result<ReqAddr, Error> {
        let mut buf = [0; 255];

        self.read_exact(&mut buf[0..5]).await?;

        let p = buf[0];
        if p != SOCKS5_PROTOCOL {
            Err(Error::from_description(&format!(
                "Not SOCKS5 protocol - {}",
                p
            )))?
        }

        let cmd = buf[1];
        if cmd != SOCKS5_CMD_CONNECT {
            self.send_final_response(Socks5Error::CommandNotSupported, ReqAddr::default())
                .await?;
            Err(Error::from_description("req cmd is not connect"))?;
        };

        let atyp = buf[3];
        let b0 = buf[4];
        let addr = match Socks5AddrType::try_from(atyp) {
            Ok(Socks5AddrType::IPV4) => {
                let addr_bytes = &mut buf[5..10];
                self.read_exact(addr_bytes).await?;
                ReqAddr::parse_address_v4(&buf[4..10])
            }
            Ok(Socks5AddrType::IPV6) => {
                let addr_bytes = &mut buf[5..22];
                self.read_exact(addr_bytes).await?;
                ReqAddr::parse_address_v6(&buf[4..22])
            }
            Ok(Socks5AddrType::DOMAIN) => {
                let addr_len = b0;
                let addr = &mut buf[0..addr_len as usize + 2];
                self.read_exact(addr).await?;
                let r = ReqAddr::parse_domain(addr_len as usize, addr);
                debug!("domain: {}", r.as_ref().unwrap());
                r
            }
            Err(e) => {
                self.send_final_response(Socks5Error::AddressTypeNotSupported, ReqAddr::default())
                    .await?;
                Err(e)
            }
        };
        if let Err(_) = addr {
            self.send_final_response(Socks5Error::AddressTypeNotSupported, ReqAddr::default())
                .await?;
        }
        addr
    }

    async fn send_final_response(&mut self, err: Socks5Error, addr: ReqAddr) -> Result<(), Error> {
        let mut resp = [0u8; 256 + 5];
        resp[0] = 5;
        resp[1] = err as u8;
        resp[2] = 0;
        let pos = match &addr {
            ReqAddr::IP(SocketAddr::V4(ref a)) => {
                resp[3] = Socks5AddrType::IPV4 as u8;
                resp[4..8].copy_from_slice(&a.ip().octets());
                8
            }
            ReqAddr::IP(SocketAddr::V6(ref a)) => {
                resp[3] = Socks5AddrType::IPV6 as u8;
                let mut pos = 4;
                for &segment in a.ip().segments().iter() {
                    resp[pos] = (segment >> 8) as u8;
                    resp[pos + 1] = segment as u8;
                    pos += 2;
                }
                pos
            }
            ReqAddr::Domain(domain, _) => {
                resp[3] = Socks5AddrType::DOMAIN as u8;
                resp[4] = domain.len() as u8;
                let pos = 5 + resp[4] as usize;
                (&mut resp[5..pos]).copy_from_slice(domain.as_bytes());
                pos
            }
        };
        resp[pos] = (addr.port() >> 8) as u8;
        resp[pos + 1] = addr.port() as u8;
        self.write_all(&resp[0..pos + 2]).await?;
        info!("final response sent code:{}", err);
        Ok(())
    }
}
