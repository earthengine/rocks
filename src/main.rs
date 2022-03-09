#![deny(bare_trait_objects)]

use clap::{Arg, Command};
use futures::Future;
use log::{error, info};
use std::io::Read;
use std::{fs::File, pin::Pin};

pub type PinboxedSendFuture<'a, O> = Pin<Box<dyn Future<Output = O> + Send + 'a>>;
pub type StandardFuture<'a, O, E> = Pin<Box<dyn Future<Output = Result<O, E>> + Send + 'a>>;

mod client_manager;
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

use config::{OutgoingConfig, RocksConfig};
use connection::bicopy;
use incoming::{get_incoming, Incoming, IncomingClient};
use outgoing::{get_outgoing, Outgoing, OutgoingError};

use crate::req_addr::ReqAddr;

async fn abort(client: impl IncomingClient + Send, error: OutgoingError, req: ReqAddr) {
    client
        .abort(error, req)
        .await
        .unwrap_or_else(|e| error!("handle_request error: {}", e));
}

async fn process_request(mut client: impl IncomingClient + Send, o: impl Outgoing, r: ReqAddr) {
    match o.process_request(r.clone()).await {
        Ok(o) => match client.ready_for_connect(r).await {
            Ok(i) => match bicopy(i, o).await {
                Err(e) => error!("error during transfer: {}", e),
                _ => (),
            },
            Err(e) => error!("can't get ready stream: {}", e),
        },
        Err(e) => abort(client, e, r).await,
    }
}

async fn handle_client(mut client: impl IncomingClient + Send, outgoing_cfg: OutgoingConfig) {
    match client.next_request().await {
        Ok(r) => match get_outgoing(outgoing_cfg) {
            Ok(o) => process_request(client, o, r).await,
            Err(e) => abort(client, OutgoingError::GeneralFailure(e), r).await,
        },
        Err(e) => {
            error!("can't handle request: {}", e)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), error::Error> {
    env_logger::init();

    let matches = Command::new("earthengineweb")
        .version("0.2.0")
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

    let mut incoming = get_incoming(conf.incoming).await?;

    loop {
        if let Ok(c) = incoming.next_client().await {
            let o_conf = conf.outgoing.clone();
            tokio::spawn(handle_client(c, o_conf));
        } else {
            break;
        }
    }
    Ok(())

    /*
    incoming.start(outgoing).await
    */
}
