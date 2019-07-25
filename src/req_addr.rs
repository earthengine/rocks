use futures::compat::Future01CompatExt;
use failure::Context;
use std::net::{SocketAddr, ToSocketAddrs, IpAddr};
use trust_dns_resolver::AsyncResolver;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ReqAddr {
    IP(SocketAddr),
    Domain(String, u16),
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
            ReqAddr::Domain(_, port) => *port,
        }
    }
    #[allow(dead_code)]
    pub fn resolve_local(&self) -> Result<SocketAddr, failure::Error> {
        match self {
            ReqAddr::IP(ip) => Ok(ip.clone()),
            ReqAddr::Domain(domain, port) => {
                (domain.as_ref(), *port)
                .to_socket_addrs()?
                .into_iter()
                .next()
                .map(|addr| SocketAddr::new(addr.ip(), *port))
                .ok_or(format_err!("Local resolve faiure"))
            }
        }
    }
    pub async fn resolve(&self, resolver: AsyncResolver) -> Result<SocketAddr, failure::Error> {
        match self {
            ReqAddr::IP(ip) => Ok(ip.clone()),
            ReqAddr::Domain(domain, port) => {
                let lookup = resolver.lookup_ip(domain.as_str()).compat().await
                    .map_err(|e| {error!("ee {}", e); e})?;
                lookup
                    .iter()
                    .next()
                    .ok_or(Context::new("Domain does not exist").into())
                    .map(|ip| SocketAddr::new(ip, *port))
            }
        }
    }
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
