//mod actix_app;
mod connection_manager;
mod hyper_svr;
mod hyper_service_helper;

use crate::config::config_rustls_server;
use crate::connection::copy;
use crate::incoming::Incoming;
use crate::outgoing::Outgoing;
use crate::tls_stream::TlsStream;
use crate::futures::StreamExt;
//use actix_app::{RocksPacket, rocks_handler, test, segqueue_future};
//use actix_web::{http::Method, server, App};
use connection_manager::ConnectionManager;
use core::{future::Future, pin::Pin};
use failure::Error;
use rustls::{ServerConfig, ServerSession};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use hyper_svr::start_server;
use futures::task::Spawn;

#[derive(Clone)]
pub struct RocksIncoming {
    ssl_conf: Arc<ServerConfig>,
    connection_manager: Arc<ConnectionManager>,
    listen_addr: SocketAddr,
    fallback_addr: SocketAddr,
}
impl Incoming for RocksIncoming {
    fn start<O,S>(self, outgoing: O,spawner: S) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>
    where
        O: Outgoing + Send + Unpin + 'static,
        S: Spawn + Send + 'static
    {
        Box::pin(self.start_impl(outgoing))
    }
}

impl RocksIncoming {
    pub fn test() -> Result<Self, Error> {
        let ssl_conf = Arc::new(config_rustls_server("key.pem", "cert.pem")?);
        let listen_addr = "127.0.0.1:8443".parse()?;
        let connection_manager = Arc::new(ConnectionManager::new());
        let fallback_addr = "127.0.0.1:8888".parse()?;

        Ok(Self {
            ssl_conf,
            connection_manager,
            listen_addr,
            fallback_addr,
        })
    }
    async fn start_impl(self, outgoing: impl Outgoing + Send + Unpin + 'static) -> Result<(), Error> {
        let listener = TcpListener::bind(&self.listen_addr)?;
        let mut incoming = listener.incoming();
        info!("listening at {:?}", self.listen_addr);
        let listen_addr = self.listen_addr;
        tokio::spawn_async(start_server(self.fallback_addr, outgoing));

        /*std::thread::spawn(move || {
            server::new(move || {
                let q = queue.clone();
                App::new()
                    .resource("/", |r| r.f(test))
                    .resource("/rocks", move |r| r.f(move |r| rocks_handler(r, q.clone())))
            })
            .bind("127.0.0.1:8888")
            .expect("Cannot bind to 127.0.0.1:8888")
            .run();
        });*/

        while let Some(stream) = incoming.next().await {
            info!("incoming!");
            let state = (self.clone(), stream);
            tokio::spawn_async(
                async move {
                    match await!(state.0.handle_client(state.1)) {
                        Err(e) => error!("{}", e),
                        Ok(_) => {}
                    }
                },
            )
        }

        Ok(())
    }
    async fn handle_client(
        mut self,
        stream: Result<TcpStream, std::io::Error>,
    ) -> Result<(), Error> {
        let ss = ServerSession::new(&self.ssl_conf);
        let s = TlsStream::new(stream?, ss);

        //let fut = segqueue_future(queue);

        let conn = await!(TcpStream::connect(&self.fallback_addr))?;
        let cpy = copy(s, conn);
        info!("connected");
        //let (p,_) = await!(causally_with(fut, cpy));
        await!(cpy)?;
        Ok(())
    }

    /*async fn process_packet(self, p: RocksPacket) -> Result<(),Error> {
        info!("process packet");
        match p {
            RocksPacket::Connect{user, addr, resp} => {
                info!("user: {}", user);
                resp.send(b"Rocks package return".to_vec());
            },
            _ => {

            }
        }
        Ok(())
    }*/
}
