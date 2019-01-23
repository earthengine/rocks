use crate::error::{Error, MyResultExt};
use crate::req_addr::ReqAddr;
use websocket::server::upgrade::r#async::Upgrade;
use trust_dns_resolver::AsyncResolver;
use tokio::net::TcpStream;
use websocket::client::r#async::TlsStream;
use tokio::prelude::{Sink, Stream};
use websocket::message::OwnedMessage;
use crate::socks5::{Socks5Error,resolver::Socks5Resolver};
use crate::connection::{ConnectionMaster,WrapFramed,copy};

#[derive(Serialize, Deserialize)]
pub struct WebSocketRequest {
    addr: ReqAddr,
    user: uuid::Uuid
}
impl WebSocketRequest {
    pub fn new(addr: ReqAddr, user: uuid::Uuid) -> Self {
        Self { addr, user }
    }
}

pub async fn serve_websocket(
    upgrade: Result<(Upgrade<TlsStream<TcpStream>>, ReqAddr, ReqAddr), Error>, 
    resolver: AsyncResolver,
    user: impl AsRef<str>,
) -> Result<(), Error> {
    let enabled_user = uuid::Uuid::parse_str(user.as_ref()).map_my_err("invalid uuid")?;
    let (upgrade, laddr, paddr) = upgrade.map_err(|e| {error!("{}", e); e})?;
    info!("laddr {:?} paddr {:?} cnt {}", laddr, paddr, upgrade.protocols().iter().count());
    if !upgrade.protocols().iter().any(|s| s=="rust-websocket") {
        await!(upgrade.reject()).map_my_err("serve_websocket")?;
        return Ok(());
    }
    let (client,_) = await!(upgrade.use_protocol("rust-websocket")
                              .accept())
                              .map_my_err("ws accept")?;  
    let (item, client) = await!(client.into_future()).map_err(|(e,_)| e).map_my_err("into_future")?;    
    if let Some(OwnedMessage::Binary(addr))=item {
        let WebSocketRequest{ addr, user } = bincode::deserialize::<'_, WebSocketRequest>(&addr).map_my_err("deserialize")?;
        if user!=enabled_user {
            return Err(Error::new("User uuid does not match"))
        }
        info!("{:?}", addr);
        let resolver = Socks5Resolver::new(resolver);
        match await!(resolver.connect_tcpstream(addr)) {
            Ok(s) => {
                let client = await!(client.send(vec![Socks5Error::Success as u8].into())).map_my_err("send back")?;
                await!(copy(WrapFramed::new(client), s.get_connection())).map_my_err("ws copy")?;
                return Ok(())
            },
            Err((r,e)) => {
                await!(client.send(vec![r as u8].into()))
                    .map_my_err("ws connect")?;
                Err(e)?
            }
        }
    }

    Err(Error::new("Not processable"))
}