use std::net::{SocketAddr, ToSocketAddrs};

use log::debug;

use crate::error::Error;
// use trust_dns_resolver::AsyncResolver;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Clone, Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub enum ReqAddr {
    IP(SocketAddr),
    Domain(String, u16),
}
impl ReqAddr {
    pub fn from_addr(addr: SocketAddr) -> Self {
        ReqAddr::IP(addr)
    }
    fn from_domain(domain: impl Into<String>, port: u16) -> Self {
        ReqAddr::Domain(domain.into(), port)
    }
    pub fn port(&self) -> u16 {
        match self {
            ReqAddr::IP(addr) => addr.port(),
            ReqAddr::Domain(_, port) => *port,
        }
    }
    pub fn parse_address_v4(addr_bytes: &[u8]) -> Result<ReqAddr, Error> {
        if addr_bytes.len() != 6 {
            if addr_bytes.len() != 6 {
                Err(Error::from_description("IPv4 address format error"))?
            }
        }
        let host = Ipv4Addr::new(addr_bytes[0], addr_bytes[1], addr_bytes[2], addr_bytes[3]);
        let port = ((addr_bytes[3] as u16) << 8) | (addr_bytes[4] as u16);
        Ok(ReqAddr::from_addr(SocketAddr::new(IpAddr::V4(host), port)))
    }
    pub fn parse_address_v6(addr_bytes: &[u8]) -> Result<ReqAddr, Error> {
        if addr_bytes.len() != 18 {
            Err(Error::from_description("IPv6 address format error"))?
        }
        let a = ((addr_bytes[0] as u16) << 8) | (addr_bytes[1] as u16);
        let b = ((addr_bytes[2] as u16) << 8) | (addr_bytes[3] as u16);
        let c = ((addr_bytes[4] as u16) << 8) | (addr_bytes[5] as u16);
        let d = ((addr_bytes[6] as u16) << 8) | (addr_bytes[7] as u16);
        let e = ((addr_bytes[8] as u16) << 8) | (addr_bytes[9] as u16);
        let f = ((addr_bytes[10] as u16) << 8) | (addr_bytes[11] as u16);
        let g = ((addr_bytes[12] as u16) << 8) | (addr_bytes[13] as u16);
        let h = ((addr_bytes[14] as u16) << 8) | (addr_bytes[15] as u16);
        let host = Ipv6Addr::new(a, b, c, d, e, f, g, h);
        let port = ((addr_bytes[16] as u16) << 8) | (addr_bytes[17] as u16);
        Ok(ReqAddr::from_addr(SocketAddr::new(IpAddr::V6(host), port)))
    }
    pub fn parse_domain(addr_len: usize, addr_bytes: &[u8]) -> Result<ReqAddr, Error> {
        let hostname = std::str::from_utf8(&addr_bytes[0..addr_len])?.to_string();
        let port = ((addr_bytes[addr_len] as u16) << 8) | (addr_bytes[addr_len + 1] as u16);
        Ok(ReqAddr::from_domain(hostname, port))
    }

    //     #[allow(dead_code)]
    pub fn resolve_local(&self) -> Result<SocketAddr, Error> {
        match self {
            ReqAddr::IP(ip) => Ok(ip.clone()),
            ReqAddr::Domain(domain, port) => {
                let sas = (domain.as_ref(), *port)
                    .to_socket_addrs()?
                    .into_iter()
                    .next();
                debug!("sas: {:?}", sas);
                sas.map(|addr| SocketAddr::new(addr.ip(), *port))
                    .ok_or(Error::from_description("Local resolve faiure"))
            }
        }
    }
    //     pub async fn resolve(&self, resolver: AsyncResolver) -> Result<SocketAddr, failure::Error> {
    //         match self {
    //             ReqAddr::IP(ip) => Ok(ip.clone()),
    //             ReqAddr::Domain(domain, port) => {
    //                 let lookup = resolver.lookup_ip(domain.as_str()).compat().await
    //                     .map_err(|e| {error!("ee {}", e); e})?;
    //                 lookup
    //                     .iter()
    //                     .next()
    //                     .ok_or(Context::new("Domain does not exist").into())
    //                     .map(|ip| SocketAddr::new(ip, *port))
    //             }
    //         }
    //     }
}
impl Default for ReqAddr {
    fn default() -> Self {
        ReqAddr::IP("127.0.0.1:0".parse().unwrap())
    }
}
impl std::fmt::Display for ReqAddr {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            ReqAddr::IP(addr) => write!(fmt, "{}:{}", addr.ip(), addr.port()),
            ReqAddr::Domain(domain, port) => write!(fmt, "{}:{}", domain, port),
        }
    }
}
