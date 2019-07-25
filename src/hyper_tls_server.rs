use crate::config::{Password, ServerConfig};
use crate::req_addr::ReqAddr;
use std::fs::File;
use std::io::Read;
use failure::Error;
use native_tls::{Identity, TlsAcceptor, };
use trust_dns_resolver::AsyncResolver;
use tokio::net::TcpListener;
use crate::connection::resolve_addr;
use hyper::server::Server;
use tokio::prelude::*;
use tokio::prelude::Future as TokioFuture;
use hyper::{Request,Response,Body};
use std::future::Future as StdFuture;
use std::marker::Unpin;
use std::task::Poll::{Ready,Pending};
use std::future::poll_with_tls_waker;
use tokio::prelude::Poll;
use std::pin::Pin;
use tokio::prelude::Async::{Ready as TokioReady, NotReady};
use tokio::prelude::future::ok;
use hyper::body::Payload;
use crate::cycbuffer::LineParser;
use std::sync::{Arc,RwLock};
use std::io::{BufRead,BufReader};
use crate::stream_wrap::StreamWrapper;
use http::version::Version;
use std::ops::Deref;
use hyper::service::{MakeService,Service};
use std::marker::PhantomData;
use tokio::net::tcp::Incoming;
use tokio_tls::TlsAcceptor as TokioTlsAccepter;
use crate::hyper_service_helper::{service_fn_mut};

fn tls_acceptor(priv_key: impl AsRef<str>, pwd: Password) -> Result<tokio_tls::TlsAcceptor, Error> {
    let mut file = File::open(priv_key.as_ref())?;
    let mut identity = vec![];
    file.read_to_end(&mut identity)?;
    let identity = Identity::from_pkcs12(&identity, &pwd.pass)?;
    let acceptor = TlsAcceptor::new(identity)?;
    Ok(acceptor.into())
}

async fn temp(mut incoming: tokio::net::tcp::Incoming, acceptor: tokio_tls::TlsAcceptor) -> Result<(), Error> {
    while let Some(stream) = await!(incoming.next()) {
        let stream = stream?;
        let mut stream = await!(acceptor.accept(stream))?;
        info!("accepted!");

        let mut buf = [0u8; 4096];
        let mut lp = LineParser::new();
        let mut data:Vec<u8> = vec![];
        let mut line_cnt = 0usize;
        'a: loop {
            let c = await!(stream.read_async(&mut buf))
                .map_err(|e| dbg!(e))?;
            info!("read {} bytes", c);
            dbg!(String::from_utf8(buf[0..c].to_vec()));
            let mut s = 0;
            loop {
                let r = lp.parse_line(&buf[s..c]);
                if let Err(_) = r { break; }

                let (line, n) = r.unwrap();
                s += n;
                line_cnt+=1;
                if line==b"" {
                    data.extend(&buf[0..s]);
                    break 'a; 
                }
            }
            data.extend(&buf[0..c]);
        }
        let mut hs = vec![httparse::EMPTY_HEADER; line_cnt];
        let mut req = httparse::Request::new(&mut hs);
        req.parse(&data)?;

        let req = match (req.method, req.path, req.version) {
            (Some(method), Some(path), version) => {
                info!("{} {} {:?}", method, path, version);
                let version_str = match version {
                    Some(0) => "HTTP/1.0",
                    Some(1) => "HTTP/1.1",
                    _ => ""
                };
                let version = match version {
                    Some(0) => Version::HTTP_10,
                    Some(1) => {
                        if let Some(_) = req.headers.iter().filter(|h| h.name=="Upgrade" && h.value==b"h2c").next()
                        { Version::HTTP_2 } else { Version::HTTP_11 }
                    },
                    _ => Version::HTTP_09,
                };
                let mut bd = hyper::Request::builder();
                bd.version(version)
                    .method(method)
                    .uri(path);
                for header in req.headers.iter().filter(|h| **h!=httparse::EMPTY_HEADER) {
                    let hv = header.value.to_vec();
                    info!("{:?}", String::from_utf8(hv.clone()));
                    let hv:&[u8] = &hv;
                    bd.header(header.name, hv);
                }

                Ok(())
            },
            _ => {
                Err(Error::from(failure::Context::new("Incomplete request")))
            }
        };
        req?;

        await!(stream.write_all_async(b"HTTP/1.1 200 OK\r\n\r\nHello"))?;
    }
    Ok(())
}

async fn hyper_serving(incoming: Incoming, acceptor: TokioTlsAccepter) -> Result<(), Error>
{
    let incoming = incoming.map_err(Error::from)
                           .and_then(|stream| {
                               info!("incoming!");
                               acceptor.accept(stream).map_err(Error::from)
                           });
    let server = Server::builder(incoming)
                    .serve(|| -> Result<_,Error> {Ok(service_fn_mut(|req: Request<Body>| {
                        if req.version() == Version::HTTP_11 {
                            Ok(Response::new(Body::from("Hello World")))
                        } else {
                            // Note: it's usually better to return a Response
                            // with an appropriate StatusCode instead of an Err.
                            Err("not HTTP/1.1, abort connection")
                        }
                    }))});
    await!(server)?;
    Ok(())
}

pub async fn serving(resolver: AsyncResolver, conf: ServerConfig) -> Result<(), Error> {
    let listen_addr = ReqAddr::try_from_cfg(conf.listen_addr)?;
    info!("serving! {}", listen_addr);
    let listen_addr = await!(resolve_addr(resolver.clone(), listen_addr))?
                    .ok_or_else(|| failure::Context::new("domain unresolvable"))?;

    info!("{}", conf.keyfile);
    let acceptor = tls_acceptor(conf.keyfile, conf.pass)?;
    let listener = TcpListener::bind(&listen_addr)?;

    let userfile = conf.userfile;
    let incoming = listener.incoming();

    await!(hyper_serving(incoming, acceptor))?;

    Ok(())
}