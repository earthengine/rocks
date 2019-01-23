use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::prelude::{AsyncReadExt, AsyncWriteExt, Async};
use std::io::{Read, Write};
use websocket::client::r#async::{Framed,TlsStream};

use crate::error::{Error,MyResultExt};
use crate::req_addr::ReqAddr;
use crate::future_ext::join;

pub trait Connection: AsyncRead + AsyncWrite
{
    fn l_addr(&self) -> Result<ReqAddr, Error>;
    fn p_addr(&self) -> Result<ReqAddr, Error>;
}
pub trait ConnectionMaker<'a> {
    type Target: Connection + Copy + Send;
}
pub trait ConnectionMaster: for<'a> ConnectionMaker<'a> + AsyncRead + AsyncWrite {
    fn get_connection<'a>(&'a self) -> <Self as ConnectionMaker<'a>>::Target;
}
impl Connection for &TcpStream {
    fn l_addr(&self) -> Result<ReqAddr, Error> {
        self.local_addr()
            .map(|addr| ReqAddr::from_addr(addr))
            .map_my_err("local addr")
    }
    fn p_addr(&self) -> Result<ReqAddr, Error> {
        self.peer_addr()
            .map(|addr| ReqAddr::from_addr(addr))
            .map_my_err("local addr")
    }
}
impl<'a> ConnectionMaker<'a> for TcpStream {
    type Target = &'a Self;
}
impl ConnectionMaster for TcpStream {
    fn get_connection<'a>(&'a self) -> <Self as ConnectionMaker<'a>>::Target {
        self
    } 
}
impl<C> Connection for TlsStream<C>
where C: ConnectionMaster
{
    fn l_addr(&self) -> Result<ReqAddr, Error> {
        self.get_ref().get_ref().get_connection().l_addr()
    }
    fn p_addr(&self) -> Result<ReqAddr, Error> {
        self.get_ref().get_ref().get_connection().p_addr()
    }
}

pub struct WrapFramed<C,E>(Framed<C,E>);
impl<C,E> WrapFramed<C,E> {
    pub fn new(f: Framed<C,E>) -> Self {
        WrapFramed(f)
    }
}
impl<C,E> Read for WrapFramed<C,E> where C: Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        self.0.get_mut().read(buf)
    }
}
impl<C,E> AsyncRead for WrapFramed<C,E> where C: AsyncRead {}
impl<C,E> Write for WrapFramed<C,E> where C: Write {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.0.get_mut().write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.0.get_mut().flush()
    }
}
impl<C,E> AsyncWrite for WrapFramed<C,E> where C: AsyncWrite {
    fn shutdown(&mut self) -> Result<Async<()>, std::io::Error> {
        self.0.get_mut().shutdown()
    }
}

impl<C,E> Connection for WrapFramed<C,E>
where C: Connection
{
    fn l_addr(&self) -> Result<ReqAddr, Error> {
        self.0.get_ref().l_addr()
    }
    fn p_addr(&self) -> Result<ReqAddr, Error> {
        self.0.get_ref().p_addr()
    }
}

async fn transfer(
    mut r: impl AsyncRead,
    mut w: impl AsyncWrite,
    peer_ports: (u16, u16),
    other_ports: (u16, u16),
    stopped: std::sync::Arc<std::sync::RwLock<bool>>
) -> Result<(), Error> {
    let mut buf = [0u8; 32768];
    let mut total = 0;
    loop {
        if *stopped.read().map_err(|e| 
                error!("stop flag poisoned: {}", e)                
            ).unwrap() {
            debug!(
                "shutdown flag set ({} -> {} | {} -> {}) total bytes: {}",
                peer_ports.0, peer_ports.1, other_ports.0, other_ports.1, total
            );
            w.shutdown().map_my_err("shutdown")?;
            break;
        }
        let cnt = await!(r.read_async(&mut buf)).map_my_err(
            format!("({} -> {} | {} -> {}) read", peer_ports.0, peer_ports.1, other_ports.0, other_ports.1))?;
        debug!(
            "{}/{} bytes received ({} -> {} | {} -> {})",
            cnt, total, peer_ports.0, peer_ports.1, other_ports.0, other_ports.1, 
        );
        total += cnt;
        if cnt == 0 || *stopped.read().map_err(|e| 
                error!("stop flag poisoned: {}", e)                
            ).unwrap() {
            debug!(
                "shutdown ({} -> {} | {} -> {}) total bytes: {}",
                peer_ports.0, peer_ports.1, other_ports.0, other_ports.1, total
            );
            w.shutdown().map_my_err("shutdown")?;
            break;
        }
        await!(w.write_all_async(&buf[0..cnt])).map_my_err(format!(
            "write ({} -> {} | {} -> {}) {}/{}",
            peer_ports.0, peer_ports.1, other_ports.0, other_ports.1, cnt, total
        ))?;
    }
    Ok(())
}

pub async fn copy(incoming: impl Connection, outgoing: impl Connection) -> Result<(), Error> {
    let riport = incoming.p_addr()?.port();
    let loport = outgoing.l_addr()?.port();

    let roport = incoming.l_addr()?.port();
    let liport = outgoing.p_addr()?.port();
    info!("({} -> {} | {} -> {})", riport, roport, loport, liport);

    let (rin, win) = incoming.split();
    let (rout, wout) = outgoing.split();

    let flag = std::sync::Arc::new(std::sync::RwLock::new(false));
    let i2o = transfer(rin, wout, (riport, roport), (loport, liport), flag.clone());
    let o2i = transfer(rout, win, (loport, liport), (riport, roport), flag.clone());
    let (r1,r2) = await!(join(
        i2o, o2i,        
    ));
    if let Err(e)=r1 {
        error!("copy error first half: {}", e);
        *flag.write().unwrap() = true;
    }
    if let Err(e)=r2 {
        error!("copy error second half: {}", e);
        *flag.write().unwrap() = true;
    }
    Ok(())
}