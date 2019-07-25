use crate::connection::Connection;
use failure::Error;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

pub struct ConnectionManager {
    conns: Mutex<HashMap<Uuid, Box<dyn Connection + Send + 'static>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        ConnectionManager {
            conns: Mutex::new(HashMap::new()),
        }
    }
    pub fn get_connection<'a>(
        &'a self,
        conn_id: Uuid,
    ) -> Result<Box<dyn Connection + Send + 'static>, Error> {
        match self.conns.lock() {
            Ok(mut conns) => {
                if conns.contains_key(&conn_id) {
                    if let Some(conn) = conns.remove(&conn_id) {
                        return Ok(conn);
                    } else {
                        bail!("No connections found")
                    }
                }
                bail!("Not implemented");
            }
            Err(e) => {
                bail!("Mutex poisoned");
            }
        }
    }
    pub fn add_connection(
        &self,
        conn_id: Uuid,
        conn: impl Connection + Send + 'static,
    ) -> Result<(), Error> {
        match self.conns.lock() {
            Ok(mut conns) => {
                if conns.contains_key(&conn_id) {
                    bail!("Connection exists");
                } else {
                    conns.insert(conn_id, Box::new(conn));
                    return Ok(());
                }
            }
            Err(e) => {
                bail!("Mutex poisoned");
            }
        }
    }
}
