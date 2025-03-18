use std::{collections::HashMap, convert::Infallible, sync::Arc};

use tokio::sync::{mpsc, Mutex};
use tungstenite::connect;
use url::Url;
use warp::{ws::Message, Filter, Rejection};

mod handlers;
mod models;
mod workers;
mod ws;

static BINANCE_WS_API: &str = "wss://stream.binance.com:9443";

#[derive(Debug, Clone)]
pub struct Client {
    pub client_id: String,
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>
}

type Clients = Arc<Mutex<HashMap<String, Client>>>;
type Result<T> = std::result::Result<T, Rejection>;

#[tokio::main]
async fn main() {
    let binance_url = format!(
        "{}/stream?streams=ethbtc@depth5@100ms/bnbeth@depth5@100ms",
        BINANCE_WS_API
    );
    let (mut socket, response) =
        connect(Url::parse(&binance_url).unwrap()).expect("Can't connect.");
    
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    println!("Configuring websocket route");
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(with_clients(clients.clone()))
        .and_then(handlers::ws_handler);

    let routes = ws_route.with(warp::cors().allow_any_origin());

    println!("Starting update loop");
    tokio::task::spawn(async move {
        workers::main_worker(clients.clone(), socket).await;
    });

    println!("Starting server");
    warp::serve(routes).run(([127,0,0,1], 8000)).await;
}

fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
    warp::any().map(move || clients.clone())
}

fn calc_triangle_step(
    trade_amount: f64,
    ask_price: f64,
    bid_price: f64,
    pair_name: &str,
    triangle_part: &str,
) -> f64 {
    // Compare first part of the part to the part of the triangle
    // to determine on what side of the trade we should be
    if pair_name[..triangle_part.len()] == *triangle_part {
        // sell side
        trade_amount * bid_price
    } else {
        // buy side
        trade_amount / ask_price
    }
}
