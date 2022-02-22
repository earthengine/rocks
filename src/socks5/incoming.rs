use log::{debug, error, info};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::connection::{bicopy, Connection};
use crate::outgoing::OutgoingError;
use crate::socks5::{
    Socks5AddrType, Socks5Error, SOCKS5_CMD_CONNECT, SOCKS5_NO_ACCEPTABLE_METHOD, SOCKS5_NO_AUTH,
    SOCKS5_PROTOCOL,
};
use crate::{
    config::CfgAddr, error::Error, incoming::Incoming, outgoing::Outgoing, req_addr::ReqAddr,
};
use std::convert::TryFrom;
use std::{future::Future, net::SocketAddr, pin::Pin};

#[derive(Clone, Debug)]
pub(crate) struct Socks5Incoming {
    listen_addr: SocketAddr,
}

struct Socks5Connected {
    _local_addr: SocketAddr,
    _remote_addr: SocketAddr,
    stream: TcpStream,
}

impl<'a> Incoming for Socks5Incoming {
    fn start<O>(self, outgoing: O) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>
    where
        O: Outgoing + Send + 'static,
    {
        Box::pin(self.start_impl(outgoing))
    }
}

impl Socks5Incoming {
    pub fn from_cfg(conf: CfgAddr) -> Result<Self, Error> {
        Ok(Socks5Incoming {
            listen_addr: conf.into_addr()?,
        })
    }
    async fn start_impl<O>(self, outgoing: O) -> Result<(), Error>
    where
        O: Outgoing + Send + 'static,
    {
        let listener = TcpListener::bind(&self.listen_addr).await?;
        info!("listening at {:?}", self.listen_addr);

        loop {
            let (stream, incoming_addr) = listener.accept().await?;
            info!("incoming!");
            let st = Socks5Connected {
                _local_addr: self.listen_addr,
                _remote_addr: incoming_addr,
                stream,
            };
            let o = outgoing.clone();
            tokio::spawn(async move {
                st.handle_client(o)
                    .await
                    .map_err(|e| error!("{}", e))
                    .unwrap_or(())
            });
        }
    }
}

impl Socks5Connected {
    pub async fn handle_client<O>(mut self, outgoing: O) -> Result<(), Error>
    where
        O: Outgoing + Send + 'static,
    {
        let o = outgoing.clone();
        self.authenticate_client().await?;
        info!("client authenticated");
        let req = self.get_request().await?;
        info!("request processed");
        match o.process_request(req.clone()).await {
            Ok(outgoing_stream) => {
                let addr = outgoing_stream.p_addr();
                match addr {
                    Ok(addr) => self.send_final_response(Socks5Error::Success, addr).await?,
                    Err(e) => {
                        self.send_final_response(Socks5Error::GeneralProxyFailure, req)
                            .await?;
                        Err(e)?
                    }
                };
                bicopy(self.stream, outgoing_stream).await?;
            }
            Err(e) => {
                self.send_final_response(self.get_reason(&e), req).await?;
                Err(Box::new(e))?
            }
        };
        Ok(())
    }

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

    async fn authenticate_client(&mut self) -> Result<(), Error> {
        let mut buf = [0u8; 256];
        info!("authenticate client");
        self.stream.read_exact(&mut buf[0..2]).await?;
        let p = buf[0];
        if p != SOCKS5_PROTOCOL {
            Err(Error::from_description(&format!(
                "Not SOCKS5 protocol - {}",
                p
            )))?;
        }

        let num_auth_methods = buf[1];
        self.stream
            .read_exact(&mut buf[0..num_auth_methods as usize])
            .await?;
        let authenticate_methods = &mut buf[0..num_auth_methods as usize];
        if !authenticate_methods.contains(&SOCKS5_NO_AUTH) {
            self.stream
                .write_all(&[SOCKS5_PROTOCOL, SOCKS5_NO_ACCEPTABLE_METHOD])
                .await?;
            Err(Error::from_description("No supported method given"))?;
        }
        info!(
            "client authenticate successfully ({} -> {})",
            self.stream.l_addr()?,
            self.stream.p_addr()?
        );
        self.stream
            .write_all(&[SOCKS5_PROTOCOL, SOCKS5_NO_AUTH])
            .await?;
        Ok(())
    }

    pub async fn get_request(&mut self) -> Result<ReqAddr, Error> {
        let mut buf = [0; 255];

        self.stream.read_exact(&mut buf[0..5]).await?;

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
                self.stream.read_exact(addr_bytes).await?;
                ReqAddr::parse_address_v4(&buf[4..10])
            }
            Ok(Socks5AddrType::IPV6) => {
                let addr_bytes = &mut buf[5..22];
                self.stream.read_exact(addr_bytes).await?;
                ReqAddr::parse_address_v6(&buf[4..22])
            }
            Ok(Socks5AddrType::DOMAIN) => {
                let addr_len = b0;
                let addr = &mut buf[0..addr_len as usize + 2];
                self.stream.read_exact(addr).await?;
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
        self.stream.write_all(&resp[0..pos + 2]).await?;
        info!("final response sent code:{}", err);
        Ok(())
    }
}
