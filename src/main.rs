// #![feature(await_macro, async_await, never_type, gen_future)]
//#![feature(, )]
#![deny(bare_trait_objects)]

use clap::{Arg, Command};
use log::info;
use std::fs::File;
use std::io::Read;

mod config;
mod connection;
mod error;
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

#[tokio::main]
async fn main() -> Result<(), error::Error> {
    env_logger::init();

    let matches = Command::new("earthengineweb")
        .version("0.1.0")
        .about("Earth Engine's web site")
        .author("Earth Engine")
        .arg(
            Arg::new("config")
                .short('c')
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

    let incoming = get_incoming(conf.incoming)?;
    let outgoing = get_outgoing(conf.outgoing)?;

    incoming.start(outgoing).await
}
