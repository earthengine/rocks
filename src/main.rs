#![feature(await_macro, async_await, futures_api)]
#![deny(bare_trait_objects)]
#![recursion_limit = "128"]

#[macro_use] extern crate tokio;
extern crate failure;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate native_tls;
extern crate tokio_tls;
extern crate trust_dns_resolver;
extern crate websocket;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate bincode;
extern crate uuid;

use tokio::net::TcpListener;
use tokio::prelude::*;
use websocket::{server::WsServer, r#async::Handle};

use std::net::SocketAddr;
use std::fs::File;
use std::env;
use native_tls::{Identity, TlsAcceptor};

mod future_ext;
mod error;
mod socks5;
mod connection;
mod req_addr;
mod wsocket;

use future_ext::{join,wrap_future};
use error::MyResultExt;
use wsocket::serve_websocket;
use socks5::serve_socks5;
use req_addr::ReqAddr;

use trust_dns_resolver::{
    config::{ResolverConfig, ResolverOpts},
    AsyncResolver,
};
fn main() -> Result<(), failure::Error> {
    env_logger::init();

    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8443".to_string());
    let addr = addr.parse::<SocketAddr>()?;

    // Bind the TCP listener
    let listener = TcpListener::bind(&addr)?;
    info!("Listening on: {}", addr);

    let wsaddr = env::args().nth(2).unwrap_or("127.0.0.1:8444".to_string());
    let wsaddr = wsaddr.parse::<SocketAddr>()?;

    let mut file = File::open("localtest.pfx")?;
    let mut identity = vec![];
    file.read_to_end(&mut identity)?;
    let identity = Identity::from_pkcs12(&identity, "12345678")?;
    
    let acceptor = TlsAcceptor::new(identity)?;
    let wsserver = WsServer::<TlsAcceptor, tokio::net::TcpListener>
                ::bind_secure(&wsaddr, acceptor, &Handle::default())?;
    let (resolver, rf) =
        AsyncResolver::new(ResolverConfig::google(), ResolverOpts::default());
    let wsresolver = resolver.clone();
    let userid = "e80e2cec-61a4-408b-aaa6-0b41327b607a";
    Ok(tokio::run_async( async move{ 
        tokio::spawn(rf);
        await!(join(async move {
            let mut incoming = listener.incoming();

            while let Some(stream) = await!(incoming.next()) {
                let port = stream
                    .as_ref()
                    .map_err(|e| format!("{}", e))
                    .and_then(|stream| stream.peer_addr().map_err(|e| format!("{}", e)));
                info!("incoming! {:?}", port);
                let resolver = resolver.clone();

                tokio::spawn_async(wrap_future(
                    serve_socks5(stream.map_my_err("stream"), resolver.clone(), userid),
                    "spawn",
                ));
            }
        }, async move {
            let mut incoming = wsserver.incoming();
            while let Some(stream) = await!(incoming.next()) {
                let upgrade = stream.map(move |(upgrade, paddr)| {
                    let laddr = wsaddr.clone();
                    (upgrade,
                        ReqAddr::from_addr(laddr), ReqAddr::from_addr(paddr))
                }).map_err(|e|e.error);
                tokio::spawn_async(wrap_future(
                    serve_websocket(upgrade.map_my_err("ws stream"), wsresolver.clone(), userid),
                    "spawn",
                ));
            }
        }));
    }))
}
