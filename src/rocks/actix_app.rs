use crate::req_addr::ReqAddr;
use actix_web::actix::fut::err;
use actix_web::{http::Method, AsyncResponder, HttpMessage, HttpRequest, Responder};
use bytes::Bytes;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::LocalWaker;
use core::task::Poll;
use core::task::Poll::Pending;
use core::task::Poll::Ready;
use crossbeam::{
    channel::{bounded, Receiver, Sender},
    queue::SegQueue,
};
use failure::Error;
use std::collections::HashMap;
use std::future::poll_with_tls_waker;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::task::Wake;
use std::time::{Duration, Instant};
use tokio::prelude::Async;
use tokio::prelude::Future as TokioFuture;
use tokio::timer::Timeout;
use uuid::Uuid;

#[derive(Debug)]
pub enum RocksPacket {
    Get {
        conn: Uuid,
        resp: Sender<Vec<u8>>,
    },
    Post {
        conn: Uuid,
        resp: Sender<Vec<u8>>,
    },
    Connect {
        user: Uuid,
        addr: ReqAddr,
        resp: Sender<Vec<u8>>,
    },
}
impl RocksPacket {
    fn new(m: Method, map: HashMap<String, String>, resp: Sender<Vec<u8>>) -> Result<Self, Error> {
        let m = (
            m,
            map.get("user"),
            map.get("host"),
            map.get("port"),
            map.get("conn"),
        );
        match m {
            (_, Some(user), Some(host), Some(port), None) => {
                let user = Uuid::parse_str(user)?;
                let port = port.parse::<u16>()?;
                match IpAddr::from_str(host) {
                    Ok(host) => Ok(RocksPacket::Connect {
                        user,
                        addr: ReqAddr::from_addr(SocketAddr::new(host, port)),
                        resp,
                    }),
                    Err(e) => Ok(RocksPacket::Connect {
                        user,
                        addr: ReqAddr::from_domain(host.clone(), port),
                        resp,
                    }),
                }
            }
            (Method::GET, None, None, None, Some(conn)) => Ok(RocksPacket::Get {
                conn: Uuid::parse_str(conn)?,
                resp,
            }),
            (Method::POST, None, None, None, Some(conn)) => Ok(RocksPacket::Post {
                conn: Uuid::parse_str(conn)?,
                resp,
            }),
            _ => bail!("Unreconised package format"),
        }
    }
}

pub fn test(_: &HttpRequest) -> impl Responder {
    "Hello"
}

pub fn rocks_handler_impl(
    req: &HttpRequest,
    queue: Arc<SegQueue<RocksPacket>>,
) -> impl tokio::prelude::Future<Item = Vec<u8>, Error = Error> {
    info!("rocks handler");
    let method = req.method().clone();
    let map = req.urlencoded().map_err(Error::from);
    let receiver = map.and_then(move |map| {
        dbg!(&map);
        let (sender, receiver) = bounded(1);
        let pkt = RocksPacket::new(method, map, sender)?;
        queue.push(pkt);
        Ok(wrap_future(ReceiverFuture(receiver)))
    });
    let receiver = receiver.and_then(|receiver| receiver);
    Timeout::new(receiver, Duration::from_secs(5)).map_err(|e| {
        if e.is_inner() {
            error!("inner err {}", e);
            e.into_inner().unwrap()
        } else if e.is_elapsed() {
            format_err!("elapsed")
        } else {
            let e = e.into_timer().unwrap();
            error!("timer");
            e.into()
        }
    })
}
pub fn rocks_handler(req: &HttpRequest, queue: Arc<SegQueue<RocksPacket>>) -> impl Responder {
    rocks_handler_impl(req, queue)
        .map(|v| Bytes::from(v))
        .map_err(|e| dbg!(e))
        .responder()
}

struct ReceiverFuture<T>(Receiver<T>);
impl<T> std::future::Future for ReceiverFuture<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<T> {
        match self.0.try_recv() {
            Ok(t) => Ready(t),
            Err(_) => Pending,
        }
    }
}
fn from_receiver<T>(recv: Receiver<T>) -> impl std::future::Future<Output = T> {
    ReceiverFuture(recv)
}

fn wrap_future<T, E>(
    f: impl std::future::Future<Output = T> + Unpin,
) -> impl tokio::prelude::Future<Item = T, Error = E> {
    struct FutureWrapper<F, E>(F, PhantomData<E>);
    impl<T, E, F> tokio::prelude::Future for FutureWrapper<F, E>
    where
        F: std::future::Future<Output = T> + Unpin,
    {
        type Item = T;
        type Error = E;
        fn poll(&mut self) -> Result<Async<T>, E> {
            struct DumpWaker();
            impl Wake for DumpWaker {
                fn wake(a: &Arc<Self>) {
                    info!("wake!")
                }
            }

            info!("poll std future");
            let lw = std::task::local_waker_from_nonlocal(Arc::new(DumpWaker()));
            match Pin::new(&mut self.0).poll(&lw) {
                Ready(v) => Ok(Async::Ready(v)),
                Pending => Ok(Async::NotReady),
            }
        }
    }
    FutureWrapper(f, PhantomData)
}

use std::fmt::Debug;
pub fn segqueue_future<T:Debug>(sq: Arc<SegQueue<T>>) -> impl std::future::Future<Output=T> {
    struct SegQueueFuture<T>(Arc<SegQueue<T>>);
    impl<T:Debug> std::future::Future for SegQueueFuture<T> {
        type Output = T;
        fn poll(self: Pin<&mut Self>, _: &LocalWaker) -> Poll<T> {
            info!("poll seg queue future");
            match self.into_ref().get_ref().0.as_ref().pop() {
                Ok(t) => Ready(dbg!(t)),
                Err(_) => dbg!(Pending)
            }
        }
    }
    SegQueueFuture(sq)
}
