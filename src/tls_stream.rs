use crate::connection::Connection;
use crate::req_addr::ReqAddr;
use core::task::Poll;
use failure::Error;
use rustls::{ServerSession, Session};
use std::io::{Error as IoError, ErrorKind};
use std::net::Shutdown;
use tokio::net::TcpStream;
use tokio::prelude::*;

pub struct TlsStream<S> {
    stream: TcpStream,
    session: S,
    eof: bool,
}

impl TlsStream<ServerSession> {
    pub fn new(stream: TcpStream, session: ServerSession) -> Self {
        Self {
            stream,
            session,
            eof: false,
        }
    }
}
impl<S> TlsStream<S>
where
    S: Session,
{
    fn check_read(self: &mut Self) -> Poll<Result<(), IoError>> {
        while !self.eof && self.session.wants_read() {
            match self.session.read_tls(&mut self.stream) {
                Ok(n) => {
                    info!("read tls {} bytes", n);
                    if n == 0 {
                        self.eof = true;
                        return Poll::Ready(Ok(()));
                    }
                    self.session
                        .process_new_packets()
                        .map_err(|e| IoError::new(ErrorKind::InvalidData, e))?;
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    return Poll::Pending;
                }
                Err(e) => return Poll::Ready(Err(e)),
            }
        }
        Poll::Ready(Ok(()))
    }
    fn check_write(self: &mut Self) -> Poll<Result<(), IoError>> {
        while self.session.wants_write() {
            match self.session.write_tls(&mut self.stream) {
                Ok(n) => {
                    info!("write tls {} bytes", n);
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    return Poll::Pending;
                }
                Err(e) => return Poll::Ready(Err(e)),
            }
        }
        Poll::Ready(Ok(()))
    }
    fn complete_io(self: &mut Self) -> Poll<Result<(), IoError>> {
        match (self.check_read(), self.check_write()) {
            (Poll::Ready(Ok(_)), Poll::Ready(Ok(_))) => Poll::Ready(Ok(())),
            (Poll::Ready(Err(e)), _) | (_, Poll::Ready(Err(e))) => Poll::Ready(Err(e)),
            _ => {
                if self.session.is_handshaking() {
                    Poll::Pending
                } else {
                    Poll::Ready(Ok(()))
                }
            }
        }
    }
    fn check_would_block<T>(p: Poll<Result<T, IoError>>) -> Result<T, IoError> {
        match p? {
            Poll::Ready(t) => Ok(t),
            Poll::Pending => Err(IoError::from(ErrorKind::WouldBlock)),
        }
    }
}

impl<S> Read for TlsStream<S>
where
    S: Session,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        Self::check_would_block(self.complete_io())?;

        if self.eof {
            return Ok(0);
        }
        let n = self.session.read(buf)?;
        if n > 0 {
            Ok(n)
        } else {
            Err(IoError::from(ErrorKind::WouldBlock))
        }
    }
}

impl<S> Write for TlsStream<S>
where
    S: Session,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> {
        if buf.len() == 0 {
            info!("write zero byte to tls!");
            self.stream.shutdown(Shutdown::Write)?;
            return Ok(0);
        }
        if self.eof {
            return Ok(0);
        }
        Self::check_would_block(self.complete_io())?;
        let n = self.session.write(buf)?;
        Self::check_would_block(self.check_write())?;
        Ok(n)
    }
    fn flush(&mut self) -> Result<(), IoError> {
        Self::check_would_block(self.check_write())
    }
}
impl<S> AsyncRead for TlsStream<S> where S: Session {}

impl<S> AsyncWrite for TlsStream<S>
where
    S: Session,
{
    fn shutdown(&mut self) -> Result<Async<()>, IoError> {
        <TcpStream as AsyncWrite>::shutdown(&mut self.stream)?;
        match self.complete_io() {
            Poll::Pending => Ok(Async::NotReady),
            Poll::Ready(Ok(())) => Ok(Async::Ready(())),
            Poll::Ready(Err(e)) => Err(e),
        }
    }
}

impl<S> Connection for TlsStream<S>
where
    S: Session,
{
    fn l_addr(&self) -> Result<ReqAddr, Error> {
        Ok(self.stream.local_addr().map(ReqAddr::from_addr)?)
    }
    fn p_addr(&self) -> Result<ReqAddr, Error> {
        Ok(self.stream.peer_addr().map(ReqAddr::from_addr)?)
    }
}
