use crate::outgoing::OutgoingError;
use std::str::Utf8Error;

#[derive(derive_more::Display, derive_more::From, Debug)]
pub enum Error {
    Io(std::io::Error),
    Config(toml::de::Error),
    Addr(std::net::AddrParseError),
    Description(String),
    Outgoing(Box<OutgoingError>),
    Utf8(Utf8Error),
    #[display(fmt = "Invalid SOCKS5 address type")]
    InvalidSocks5AddrType,
}

impl Error {
    pub(crate) fn from_description(descriotion: &str) -> Self {
        From::<String>::from(descriotion.into())
    }
}
