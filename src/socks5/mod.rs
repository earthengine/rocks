use std::convert::TryFrom;

pub(crate) use incoming::Socks5Incoming;

mod incoming;

use crate::error::Error;

const SOCKS5_PROTOCOL: u8 = 5;
#[derive(PartialEq, Eq, derive_more::Display, Copy, Clone)]
pub enum Socks5Error {
    Success = 0,
    GeneralProxyFailure = 1,
    //ConnectionNotAllowed = 2,
    NetworkUnreachable = 3,
    HostUnreachable = 4,
    ConnectionRefused = 5,
    TTLExpired = 6,
    CommandNotSupported = 7,
    AddressTypeNotSupported = 8,
    //Unknown = 255,
}

const SOCKS5_NO_AUTH: u8 = 0;
const SOCKS5_NO_ACCEPTABLE_METHOD: u8 = 0xff;

pub enum Socks5AddrType {
    IPV4 = 1,
    IPV6 = 4,
    DOMAIN = 3,
}
impl TryFrom<u8> for Socks5AddrType {
    type Error = Error;
    fn try_from(i: u8) -> Result<Self, Self::Error> {
        match i {
            1 => Ok(Socks5AddrType::IPV4),
            3 => Ok(Socks5AddrType::DOMAIN),
            4 => Ok(Socks5AddrType::IPV6),
            _ => Err(Error::InvalidSocks5AddrType),
        }
    }
}

const SOCKS5_CMD_CONNECT: u8 = 1;
