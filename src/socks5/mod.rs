pub mod resolver;

use crate::socks5::resolver::Socks5Resolver;
use crate::{
    error::{Error, MyResultExt},
    future_ext::alt,
};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::{Duration, Instant};
use tokio::io::{read_exact, write_all};
use tokio::net::TcpStream;
use tokio::timer::Delay;
use trust_dns_resolver::AsyncResolver;


use crate::connection::Connection;
use crate::req_addr::ReqAddr;

const SOCKS5_PROTOCOL: u8 = 5;
const SOCKS5_NO_AUTH: u8 = 0;
const SOCKS5_CMD_CONNECT: u8 = 1;
const SOCKS5_ATYP_IPV4: u8 = 1;
const SOCKS5_ATYP_IPV6: u8 = 4;
const SOCKS5_ATYP_DOMAIN: u8 = 3;
const SOCKS5_NO_ACCEPTABLE_METHOD: u8 = 0xff;

#[derive(PartialEq, Eq)]
pub enum Socks5Error {
    Success = 0,
    GeneralProxyFailure = 1,
    ConnectionNotAllowed = 2,
    NetworkUnreachable = 3,
    HostUnreachable = 4,
    ConnectionRefused = 5,
    TTLExpired = 6,
    CommandNotSupported = 7,
    AddressTypeNotSupported = 8,
    Unknown = 255
}
impl From<u8> for Socks5Error {
    fn from(b: u8) -> Self {
        match b {
            0 => Socks5Error::Success,
            1 => Socks5Error::GeneralProxyFailure,
            2 => Socks5Error::ConnectionNotAllowed,
            3 => Socks5Error::NetworkUnreachable,
            4 => Socks5Error::HostUnreachable,
            5 => Socks5Error::ConnectionRefused,
            6 => Socks5Error::TTLExpired,
            7 => Socks5Error::CommandNotSupported,
            8 => Socks5Error::AddressTypeNotSupported,            
            _ => Socks5Error::Unknown,
        }
    }
}

pub async fn serve_socks5(
    stream: Result<TcpStream, Error>,
    resolver: AsyncResolver,
    user: impl AsRef<str>,
) -> Result<(), Error> {
    let incoming = stream.map_my_err("incoming stream")?;
    let timeout = Delay::new(Instant::now() + Duration::new(10, 0));
    let user = uuid::Uuid::parse_str(user.as_ref()).map_my_err("invalid uuid")?;
    let outgoing =
        await!(alt(async { await!(timeout) }, 
            handshake(&incoming, resolver, user),))
            .1
            .ok_or(Error::new("Timed out"))
            .map_my_err("hanshake")?
            .map_my_err("hanshake internal")?;
    await!(crate::connection::copy(&incoming, outgoing))
}

async fn handshake(
    stream: impl Connection + Copy,
    resolver: AsyncResolver,
    user: uuid::Uuid,
) -> Result<impl Connection, Error> {
    await!(authenticate(stream))?;

    let mut addr = ReqAddr::default();
    macro_rules! my_err { ($e:ident) => { Err(Error::prefix("hanshake", $e)) } }
    let (error, stream_out) = match await!(resolve_request(stream, resolver, user)) {
        Ok(stream_out) => {
            match stream_out.l_addr() {
                Ok(ad) => { addr = ad; (Socks5Error::Success, Ok(stream_out)) },
                Err(e) => (Socks5Error::GeneralProxyFailure, my_err!(e)),
            }
        },
        Err((Some(e1),e2)) => { //Already handled
            (e1, Err(e2))
        },
        Err((None,e)) => { //No need to send final response as the request is incorrect
            Err(e)?
        }
    };
    await!(send_final_response(stream, error, addr))?;

    Ok(stream_out.map_my_err("handshake")?)
}

async fn authenticate(stream: impl Connection + Copy) -> Result<(), Error> {
    let mut buf = [0; 256];
    await!(read_exact(stream, &mut buf[0..2])).map_my_err("read hello initials")?;
    let p = buf[0];
    if p != SOCKS5_PROTOCOL {
        Err(Error::new(format!("Not SOCKS5 protocol - {}", p)))?
    }

    let num_auth_methods = buf[1];

    let authenticate_methods =
        await!(read_exact(stream, &mut buf[0..num_auth_methods as usize]))
            .map_my_err("read authenticate methods list")?.1;
    if !authenticate_methods.contains(&SOCKS5_NO_AUTH) {
        await!(write_all(stream, &[SOCKS5_PROTOCOL, SOCKS5_NO_ACCEPTABLE_METHOD]))
            .map_my_err("write NAM response")?;
        Err(Error::new("No supported method given"))?;
    }

    await!(write_all(stream, [SOCKS5_PROTOCOL, SOCKS5_NO_AUTH])).map_my_err("write auth resp")?;
    Ok(())
}

async fn resolve_request(
    stream: impl Connection + Copy,
    resolver: AsyncResolver,
    user: uuid::Uuid,
) -> Result<impl Connection, (Option<Socks5Error>,Error)> {
    let mut buf = [0; 255];
    await!(read_exact(stream, &mut buf[0..5]))
            .map_my_err("read req prefix")
            .map_err(|e| (None,e))?.1;

    let p = buf[0];
    if p != SOCKS5_PROTOCOL {
        Err((None,Error::new(format!("Not SOCKS5 protocol - {}", p))))?
    }

    let cmd = buf[1];
    let atyp = buf[3];
    let b0 = buf[4];
    let addr = match atyp {
        SOCKS5_ATYP_IPV4 => {
            let addr_bytes =
                await!(read_exact(stream, &mut buf[5..10]))
                    .map_my_err("read req ipv4 addr")
                    .map_err(|e| (None,e))?.1;
            let host = Ipv4Addr::new(b0, addr_bytes[0], addr_bytes[1], addr_bytes[2]);
            let port = ((addr_bytes[3] as u16) << 8) | (addr_bytes[4] as u16);
            ReqAddr::from_addr(SocketAddr::new(IpAddr::V4(host), port))
        }
        SOCKS5_ATYP_IPV6 => {            
            let addr_bytes =
                await!(read_exact(stream, &mut buf[5..22]))
                    .map_my_err("read req ipv6 addr")
                    .map_err(|e| (None,e))?.1;
            let a = ((b0 as u16) << 8) | (addr_bytes[0] as u16);
            let b = ((addr_bytes[1] as u16) << 8) | (addr_bytes[2] as u16);
            let c = ((addr_bytes[3] as u16) << 8) | (addr_bytes[4] as u16);
            let d = ((addr_bytes[5] as u16) << 8) | (addr_bytes[6] as u16);
            let e = ((addr_bytes[7] as u16) << 8) | (addr_bytes[8] as u16);
            let f = ((addr_bytes[9] as u16) << 8) | (addr_bytes[10] as u16);
            let g = ((addr_bytes[11] as u16) << 8) | (addr_bytes[12] as u16);
            let h = ((addr_bytes[13] as u16) << 8) | (addr_bytes[14] as u16);
            let host = Ipv6Addr::new(a, b, c, d, e, f, g, h);
            let port = ((addr_bytes[15] as u16) << 8) | (addr_bytes[16] as u16);
            ReqAddr::from_addr(SocketAddr::new(IpAddr::V6(host), port))
        }
        SOCKS5_ATYP_DOMAIN => {
            let addr_len = b0;
            let addr = await!(read_exact(stream, &mut buf[0..addr_len as usize + 2]))
                        .map_my_err("read domain")
                        .map_err(|e| (None,e))?.1;            
            let hostname = {
                let port_pos: usize = addr_len as usize;
                let hostname =
                    std::str::from_utf8(&addr[0..port_pos])
                        .map_my_err("host name not utf8")
                        .map_err(|e| (Some(Socks5Error::GeneralProxyFailure),e))?;
                hostname.to_string()
            };
            let port = ((addr[addr_len as usize] as u16) << 8) | (addr[addr_len as usize + 1] as u16);
            ReqAddr::from_domain(hostname, port)
        },
        _ => {
            Err((Some(Socks5Error::AddressTypeNotSupported), Error::new("ATYP not recognized")))?
        },
    };
    if cmd != SOCKS5_CMD_CONNECT {
        Err((Some(Socks5Error::CommandNotSupported), Error::new("req cmd is not connect")))?;
    }

    let resolver = Socks5Resolver::new(resolver);
    await!(resolver.connect_websocket(addr, user))
        .map_err(|(e1,e2)| (Some(e1), e2))
}

async fn send_final_response(stream:impl Connection, err: Socks5Error, addr: ReqAddr)
    -> Result<(), Error>
{
    let mut resp = [0u8; 32];
    resp[0] = 5;
    resp[1] = err as u8;
    resp[2] = 0;
    let pos = match &addr {
        ReqAddr::IP(SocketAddr::V4(ref a)) => {
            resp[3] = SOCKS5_ATYP_IPV4;
            resp[4..8].copy_from_slice(&a.ip().octets());
            8
        },
        ReqAddr::IP(SocketAddr::V6(ref a)) => {
            resp[3] = SOCKS5_ATYP_IPV6;
            let mut pos = 4;
            for &segment in a.ip().segments().iter() {
                resp[pos] = (segment >> 8) as u8;
                resp[pos + 1] = segment as u8;
                pos += 2;
            }
            pos
        },
        ReqAddr::Domain(domain, _) => {
            resp[3] = SOCKS5_ATYP_DOMAIN;
            resp[4] = domain.len() as u8;
            let pos = 5 + resp[4] as usize;
            (&mut resp[5..pos]).copy_from_slice(domain.as_bytes());
            pos
        }
    };
    resp[pos] = (addr.port() >> 8) as u8;
    resp[pos + 1] = addr.port() as u8;
    await!(write_all(stream, &resp[0..pos+2]))
        .map_my_err("write all")?;
    Ok(())
}
