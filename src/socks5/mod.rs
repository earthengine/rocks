use crate::config::CfgAddr;
use crate::incoming::Incoming;
use crate::outgoing::Outgoing;
use crate::outgoing::OutgoingError;
use core::pin::Pin;
use std::future::Future;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

mod incoming;
pub(crate) use incoming::Socks5Incoming;

const SOCKS5_PROTOCOL: u8 = 5;
#[derive(PartialEq, Eq)]
pub enum Socks5Error {
    Success = 0,
    GeneralProxyFailure = 1,
    ConnectionNotAllowed = 2,
    NetworkUnreachable = 3,
    HostUnreachable = 4,
    ConnectionRefused = 5,
    TTLExpired = 6,
    CommandNotSupported = 7,
    AddressTypeNotSupported = 8,
    Unknown = 255,
}

const SOCKS5_NO_AUTH: u8 = 0;
const SOCKS5_NO_ACCEPTABLE_METHOD: u8 = 0xff;

const SOCKS5_ATYP_IPV4: u8 = 1;
const SOCKS5_ATYP_IPV6: u8 = 4;
const SOCKS5_ATYP_DOMAIN: u8 = 3;

const SOCKS5_CMD_CONNECT: u8 = 1;



