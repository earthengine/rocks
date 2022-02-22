use crate::error::Error;
use crate::req_addr::ReqAddr;
use log::info;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

pub trait Connection: AsyncRead + AsyncWrite + Send + Unpin {
    type ReadHalf: AsyncRead + Unpin + Send;
    type WriteHalf: AsyncWrite + Unpin + Send;
    fn l_addr(&self) -> Result<ReqAddr, Error>;
    fn p_addr(&self) -> Result<ReqAddr, Error>;
    fn split(self) -> (Self::ReadHalf, Self::WriteHalf);
}

impl Connection for TcpStream {
    type ReadHalf = tokio::io::ReadHalf<TcpStream>;
    type WriteHalf = tokio::io::WriteHalf<TcpStream>;
    fn l_addr(&self) -> Result<ReqAddr, Error> {
        Ok(self.local_addr().map(|addr| ReqAddr::from_addr(addr))?)
    }
    fn p_addr(&self) -> Result<ReqAddr, Error> {
        Ok(self.peer_addr().map(|addr| ReqAddr::from_addr(addr))?)
    }
    fn split(self) -> (Self::ReadHalf, Self::WriteHalf) {
        tokio::io::split(self)
    }
}

pub async fn bicopy(incoming: impl Connection, outgoing: impl Connection) -> Result<(), Error> {
    let riport = incoming.p_addr()?.port();
    let loport = outgoing.l_addr()?.port();

    let roport = incoming.l_addr()?.port();
    let liport = outgoing.p_addr()?.port();
    info!("({} -> {} | {} -> {})", riport, roport, loport, liport);

    let (mut rin, mut win) = incoming.split();
    let (mut rout, mut wout) = outgoing.split();

    let i2o = tokio::io::copy(&mut rin, &mut wout);
    let o2i = tokio::io::copy(&mut rout, &mut win);
    let (r1, r2) = futures::join!(i2o, o2i);
    match (r1, r2) {
        (Ok(_), Ok(_)) => Ok(()),
        (Err(e), Ok(_)) => Err(Error::from_description(&format!(
            "copy error first half: {}",
            e
        ))),
        (_, Err(e)) => Err(Error::from_description(&format!(
            "copy error second half: {}",
            e
        ))),
    }
}
