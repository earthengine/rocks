use log::error;

use crate::{
    config::OutgoingConfig,
    connection::bicopy,
    incoming::IncomingClient,
    outgoing::{get_outgoing, Outgoing, OutgoingError},
    req_addr::ReqAddr,
};

async fn abort(client: impl IncomingClient + Send, error: OutgoingError, req: ReqAddr) {
    client
        .abort(error, req)
        .await
        .unwrap_or_else(|e| error!("handle_request error: {}", e));
}

async fn process_request(mut client: impl IncomingClient + Send, o: impl Outgoing, r: ReqAddr) {
    match o.process_request(r.clone()).await {
        Ok(o) => match client.ready_for_connect(r).await {
            Ok(i) => match bicopy(i, o).await {
                Err(e) => error!("error during transfer: {}", e),
                _ => (),
            },
            Err(e) => error!("can't get ready stream: {}", e),
        },
        Err(e) => abort(client, e, r).await,
    }
}

pub async fn handle_client(mut client: impl IncomingClient + Send, outgoing_cfg: OutgoingConfig) {
    match client.next_request().await {
        Ok(r) => match get_outgoing(outgoing_cfg) {
            Ok(o) => process_request(client, o, r).await,
            Err(e) => abort(client, OutgoingError::GeneralFailure(e), r).await,
        },
        Err(e) => {
            error!("can't handle request: {}", e)
        }
    }
}
