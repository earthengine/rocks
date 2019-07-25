#![feature(
    await_macro,
    async_await,
    never_type,
    gen_future
)]
//#![feature(, )]
#![deny(bare_trait_objects)]
#![recursion_limit = "128"]

#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
//extern crate actix_web;
extern crate bytes;
extern crate crossbeam;
extern crate env_logger;
extern crate rustls;
extern crate serde;
extern crate tokio_tls;
extern crate trust_dns_resolver;
extern crate uuid;
extern crate hyper;

use clap::{App, Arg};
use std::fs::File;
use std::io::Read;

mod config;
mod connection;
// mod error;
mod future_ext;
mod incoming;
mod outgoing;
mod req_addr;
// mod rocks;
mod socks5;
// mod stream_wrap;
// mod tls_stream;

use config::RocksConfig;
use incoming::{get_incoming, Incoming};
use outgoing::get_outgoing;
use trust_dns_resolver::{
    config::{ResolverConfig, ResolverOpts},
    AsyncResolver,
};
use futures::compat::Future01CompatExt;

fn new_resolver() -> (
    AsyncResolver,
    impl hyper::rt::Future<Item=(),Error=()> + Send,
) {
    AsyncResolver::new(ResolverConfig::google(), ResolverOpts::default())
}

#[tokio::main]
async fn main() -> Result<(), failure::Error> {
    env_logger::init();

    let matches = App::new("earthengineweb")
        .version("0.1.0")
        .about("Earth Engine's web site")
        .author("Earth Engine")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Use specified config file")
                .takes_value(true),
        )
        .get_matches();

    let conf = matches.value_of("config").unwrap_or("config.toml");
    info!("config file: {}", conf);
    let mut conf = File::open(conf)?;
    let conf = toml::from_slice::<RocksConfig>(&{
        let mut r = vec![];
        conf.read_to_end(&mut r)?;
        r
    })?;
    info!("config file read");

    let (resolver, rf) = new_resolver();

    let incoming = get_incoming(conf.incoming)?;
    let outgoing = get_outgoing(conf.outgoing, resolver)?;

    tokio::spawn(async move {
        rf.compat().await.unwrap_or(())
    });

    let brk = future_ext::FlagFuture::default();
    let b = brk.clone();
    ctrlc::set_handler(move || b.set())?;
    loop {
        future_ext::join(async { incoming.clone().start(outgoing.clone()).await
            .unwrap_or_else(|e| error!("Error start: {}", e)); },
            brk.clone()).await;
        if brk.is_set() { break; }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    #[test]
    fn test_rustls() {
        use crate::config::config_rustls_server;
        config_rustls_server("key.pem", "cert.pem").unwrap();
    }
}
