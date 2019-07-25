use failure::Error;
use tokio::prelude::{AsyncRead, AsyncWrite};
use tokio::{net::TcpStream, io::AsyncReadExt};
use tokio::net::tcp::split::{TcpStreamReadHalf, TcpStreamWriteHalf};

use crate::future_ext::join;
use crate::req_addr::ReqAddr;

pub trait Connection: AsyncRead + AsyncWrite + Send + Unpin {
    type ReadHalf: AsyncRead + Send + Unpin;
    type WriteHalf: AsyncWrite + Send + Unpin;
    fn l_addr(&self) -> Result<ReqAddr, Error>;
    fn p_addr(&self) -> Result<ReqAddr, Error>;
    fn split(self) -> (Self::ReadHalf, Self::WriteHalf);
}
impl Connection for TcpStream {
    type ReadHalf = TcpStreamReadHalf;
    type WriteHalf = TcpStreamWriteHalf;
    fn l_addr(&self) -> Result<ReqAddr, Error> {
        Ok(self.local_addr().map(|addr| ReqAddr::from_addr(addr))?)
    }
    fn p_addr(&self) -> Result<ReqAddr, Error> {
        Ok(self.peer_addr().map(|addr| ReqAddr::from_addr(addr))?)
    }
    fn split(self) -> (Self::ReadHalf, Self::WriteHalf) {
        TcpStream::split(self)
    }
}

pub async fn copy(incoming: impl Connection, outgoing: impl Connection) -> Result<(), Error> {
    let riport = incoming.p_addr()?.port();
    let loport = outgoing.l_addr()?.port();

    let roport = incoming.l_addr()?.port();
    let liport = outgoing.p_addr()?.port();
    info!("({} -> {} | {} -> {})", riport, roport, loport, liport);

    let (mut rin, mut win) = incoming.split();
    let (mut rout, mut wout) = outgoing.split();

    let i2o = rin.copy(&mut wout);
    let o2i = rout.copy(&mut win);
    let (r1, r2) = await!(join(i2o, o2i,));
    match (r1, r2) {
        (Ok(_),Ok(_)) => Ok(()),
        (Err(e),Ok(_)) => bail!("copy error first half: {}", e),
        (Ok(_),Err(e)) => bail!("copy error second half: {}", e),
        (Err(e1),Err(e2)) => bail!("copy error ({}, {})", e1, e2),
    }
}
