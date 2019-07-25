use std::thread::ThreadId;
use std::thread::current;
use std::future::get_task_waker;
use core::task::Waker;
use core::task::Poll;
use core::task::LocalWaker;
use std::sync::Arc;
use std::task::Wake;
use core::task::Poll::{Ready, Pending};
use tokio::prelude::Async;
use std::net::SocketAddr;
use hyper::{Response, Request, Server, Body, StatusCode, body::Payload};
use crate::outgoing::Outgoing;
use super::hyper_service_helper::service_fn_mut;
use failure::Error;
use tokio::prelude::{Future as TokioFuture, StreamAsyncExt};
use core::pin::Pin;
use core::future::Future;
use bytes::{Buf,Bytes};

pub async fn start_server(addr: SocketAddr, outgoing: impl Outgoing + Send + Unpin + 'static) {
    let waker = get_task_waker(|waker| waker.clone().into_waker());
    let tid = current().id();
    let server = Server::bind(&addr)
        .serve(move || {
        let waker = waker.clone();
        let outgoing = outgoing.clone();
        service_fn_mut(move |req|serve_rocks(outgoing.clone(), req, waker.clone(), tid))
    });
    await!(server).unwrap_or_else(|e| error!("{}", e));
}

pub fn serve_rocks(outgoing: impl Outgoing + Send + Unpin + 'static, req: Request<Body>, waker: Waker, tid: ThreadId)
-> impl TokioFuture<Item=Response<Body>, Error=Error>
{
    wrap_future(Box::pin(serve_rocks_async(outgoing, req, Foo)), waker, tid)
}
async fn serve_rocks_async(outgoing: impl Outgoing + Send + 'static, req: Request<Body>, f: Foo)
-> Result<Response<Body>,Error>
{
    if req.uri()=="/rocks" {
        await!(handle_rocks(outgoing, req))
    } else {
        await!(handle_static_files(req))
    }
}
struct Foo;
impl Foo {
    fn foo(&self) -> Option<()> { Some(()) }
}
async fn bad()->Result<!,Error> {
    let f = Foo;
    if let Some(v) = f.foo() {
        await!(async{})
    }
    Err(format_err!("!!"))
}

async fn handle_rocks(outgoing: impl Outgoing + Send + 'static, req: Request<Body>)
    -> Result<Response<Body>,Error>
{
    let (pt,mut body) = req.into_parts();
    let hdrs = pt.headers;
    let r:Result<_,Error> = if hdrs.contains_key("content-type") 
        && hdrs["content-type"]=="application/x-www-form-urlencoded" 
        && hdrs.contains_key("content-length")
    {
        let len = hdrs["content-length"].to_str()?
                    .parse::<u64>()?;
        let bln = body.content_length();
        if let Some(l)=bln {
            if len!=l {
                bail!("length not match");
            }
            while let Some(chunk) = await!(body.next()) {
            }
        }
        Ok(())
    } else {
        Ok(())
    };    

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Connection", "close")
        .body(Body::empty())
        .map_err(Error::from)
}
async fn handle_static_files(_: Request<Body>)
    -> Result<Response<Body>,Error>
{
    Response::builder()
        .header("Context-Type", "text/html")
        .status(StatusCode::OK)
        .body(Body::from("Hello"))
        .map_err(Error::from)
}
fn payload_future<P,T,E>(p: P)
    -> impl Future<Output=Result<Option<(P,T)>,E>> 
    where P: Payload<Data=T,Error=E> + Unpin,
    T: Buf + Send, E: std::error::Error + Send + Sync + 'static
{
    struct PayloadFuture<P>(Option<P>);
    impl<T,E,P> Future for PayloadFuture<P>
    where P: Payload<Data=T,Error=E> + Unpin,
          T: Buf + Send,
          E: std::error::Error + Send + Sync + 'static
    {
        type Output=Result<Option<(P,T)>,E>;
        fn poll(self: Pin<&mut Self>, _: &LocalWaker) -> Poll<Self::Output>
        {
            let pl = self.get_mut();
            match pl.0.as_mut().unwrap().poll_data() {
                Ok(Async::Ready(Some(t))) => Ready(Ok(Some((pl.0.take().unwrap(),t)))),
                Ok(Async::Ready(None)) => Ready(Ok(None)),
                Ok(Async::NotReady) => Pending,
                Err(e) => Ready(Err(e))
            }
        }
    }
    PayloadFuture(Some(p))
}

fn wrap_future<T,E>(f: impl Future<Output=Result<T,E>>, waker: Waker, tid: ThreadId) -> impl TokioFuture<Item=T, Error=E> {
    struct WrapFuture<F>(Pin<Box<F>>, Waker, ThreadId);
    impl<T,E,F> TokioFuture for WrapFuture<F>
    where F: Future<Output=Result<T,E>>
    {
        type Item=T;
        type Error=E;
        fn poll(&mut self) -> Result<Async<T>,E> {
            if current().id() == self.2 {
                info!("same thread");
                match std::future::poll_with_tls_waker(self.0.as_mut()) {
                    Ready(Ok(t)) => Ok(Async::Ready(t)),
                    Ready(Err(e)) => Err(e),
                    Pending => Ok(Async::NotReady)
                }
            } else {
                info!("need wake");
                self.1.wake();
                Ok(Async::NotReady)
            }
        }
    }  
    WrapFuture(Box::pin(f), waker, tid)
}

