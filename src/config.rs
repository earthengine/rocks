use serde_derive::{Deserialize, Serialize};
use std::net::{SocketAddr, ToSocketAddrs};

use crate::error::Error;
#[derive(Deserialize, Serialize, Debug)]
pub struct CfgAddr {
    pub ip: Option<String>,
    pub domain: Option<String>,
    pub port: u16,
}
impl CfgAddr {
    pub fn into_addr(self) -> Result<SocketAddr, Error> {
        match self {
            CfgAddr {
                ip: Some(ip),
                domain: _,
                port,
            } => Ok(SocketAddr::new(ip.parse()?, port)),
            CfgAddr {
                ip: None,
                domain: Some(domain),
                port,
            } => {
                let addr = format!("{}:{}", domain, port)
                    .to_socket_addrs()?
                    .next()
                    .ok_or(Error::from_description("invalid domain"))?;
                Ok(addr)
            }
            _ => Err(Error::from_description("invalid address in config")),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub enum IncomingType {
    Socks5,
    Rocks,
    Deny,
    Redirect,
}

#[derive(Deserialize, Serialize)]
pub struct IncomingConfig {
    pub r#type: IncomingType,
    pub userfile: Option<String>,
    pub listen_addr: CfgAddr,
    pub ssl: Option<SslConfig>,
}

#[derive(Deserialize, Serialize)]
pub enum OutgoingType {
    Rocks,
    Direct,
    Ignore,
}

#[derive(Deserialize, Serialize)]
pub struct SslConfig {
    pub keyfile: String,
    pub certfile: String,
}

#[derive(Deserialize, Serialize)]
pub struct OutgoingConfig {
    pub r#type: OutgoingType,
    pub user: Option<String>,
    pub listen_addr: Option<CfgAddr>,
}

// #[derive(Deserialize, Serialize)]
// pub struct Password {
//     pub pass: String,
// }
#[derive(Deserialize, Serialize)]
pub struct RocksConfig {
    pub incoming: IncomingConfig,
    pub outgoing: OutgoingConfig,
}

// // #[allow(dead_code)]
// enum IncomingCfg {
//     Socks5 {
//         listen_addr: SocketAddr,
//     },
//     Rocks {
//         listen_addr: SocketAddr,
//         ssl: Option<ServerConfig>,
//         userfile: Vec<Uuid>,
//     },
//     Deny,
//     Redirect {
//         listen_addr: SocketAddr,
//         connect_addr: SocketAddr,
//     },
// }
// #[allow(dead_code)]
// pub fn config_rustls_server(
//     key: impl AsRef<str>,
//     cert: impl AsRef<str>,
// ) -> Result<ServerConfig, failure::Error> {
//     let f = File::open(key.as_ref())?;
//     let mut br = BufReader::new(f);
//     let k = pemfile::pkcs8_private_keys(&mut br).map_err(|_| format_err!("Invalid key"))?;
//     if k.len() < 1 {
//         bail!("No key in file")
//     }

//     let f = File::open(cert.as_ref())?;
//     let mut br = BufReader::new(f);
//     let c = pemfile::certs(&mut br).map_err(|_| format_err!("Invalid certificate"))?;
//     if k.len() < 1 {
//         bail!("Certificate is not valid")
//     }

//     let mut cfg = ServerConfig::new(Arc::new(NoClientAuth));
//     cfg.set_single_cert(c, k[0].clone())?;
//     Ok(cfg)
// }

// #[allow(dead_code)]
// fn write_client_cfg() -> Result<(), failure::Error> {
//     let incoming = IncomingConfig {
//         listen_addr: CfgAddr {
//             ip: Some("127.0.0.1".into()),
//             domain: None,
//             port: 4080,
//         },
//         r#type: IncomingType::Socks5,
//         userfile: None,
//         ssl: None,
//     };
//     let outgoing = OutgoingConfig {
//         r#type: OutgoingType::Rocks,
//         listen_addr: Some(CfgAddr {
//             ip: None,
//             domain: Some("example.com".into()),
//             port: 8040,
//         }),
//         user: Some("user".into()),
//     };
//     let cfg = RocksConfig { incoming, outgoing };
//     let mut f = File::create("config_client_example.toml")?;

//     f.write(toml::to_string(&cfg)?.as_bytes())?;
//     Ok(())
// }
// #[allow(dead_code)]
// fn write_server_cfg() -> Result<(), failure::Error> {
//     let incoming = IncomingConfig {
//         listen_addr: CfgAddr {
//             ip: Some("127.0.0.1".into()),
//             domain: None,
//             port: 8040,
//         },
//         r#type: IncomingType::Socks5,
//         userfile: Some("userfile".into()),
//         ssl: Some(SslConfig {
//             keyfile: "keyfile".into(),
//             certfile: "certfile".into(),
//         }),
//     };
//     let outgoing = OutgoingConfig {
//         r#type: OutgoingType::Direct,
//         listen_addr: None,
//         user: None,
//     };
//     let cfg = RocksConfig { incoming, outgoing };
//     let mut f = File::create("config_server_example.toml")?;

//     f.write(toml::to_string(&cfg)?.as_bytes())?;
//     Ok(())
// }

// #[cfg(test)]
// mod test_config {
//     use crate::config::{
//         write_client_cfg, write_server_cfg, CfgAddr, IncomingConfig, IncomingType, OutgoingConfig,
//         OutgoingType, SslConfig,
//     };
//     #[test]
//     fn test_write_config() {
//         write_client_cfg().unwrap();
//         write_server_cfg().unwrap();
//     }

//     #[test]
//     fn test_incoming_config() {
//         let incoming = IncomingConfig {
//             listen_addr: CfgAddr {
//                 ip: Some("127.0.0.1".into()),
//                 domain: None,
//                 port: 4080,
//             },
//             r#type: IncomingType::Rocks,
//             userfile: Some("userfile".into()),
//             ssl: Some(SslConfig {
//                 keyfile: "keyfile".into(),
//                 certfile: "certfile".into(),
//             }),
//         };
//         assert_eq!(
//             toml::to_string(&incoming).unwrap(),
//             r#"type = "Rocks"
// userfile = "userfile"

// [listen_addr]
// ip = "127.0.0.1"
// port = 4080

// [ssl]
// keyfile = "keyfile"
// certfile = "certfile"
// "#
//         );
//     }
//     #[test]
//     fn test_outgoing_config() {
//         let outgoing = OutgoingConfig {
//             r#type: OutgoingType::Rocks,
//             listen_addr: Some(CfgAddr {
//                 ip: Some("127.0.0.1".into()),
//                 domain: None,
//                 port: 8040,
//             }),
//             user: Some("user".into()),
//         };
//         assert_eq!(
//             toml::to_string(&outgoing).unwrap(),
//             r#"type = "Rocks"
// user = "user"

// [listen_addr]
// ip = "127.0.0.1"
// port = 8040
// "#
//         );
//     }
// }
