use warp::Reply;

use crate::{ws, Clients, Result};

pub async fn ws_handler(ws: warp::ws::Ws, clients: Clients ) -> Result<impl Reply> {
  println!("ws_handler");

  Ok(ws.on_upgrade(move |socket| ws::client_connection(socket)))
}