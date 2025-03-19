use crate::{models, Clients};
use chrono::{DateTime, Utc};
use rand::prelude::*;
use serde::Serialize;
use tokio;
use tokio::time::Duration;
use tungstenite::{client::AutoStream, WebSocket};
use warp::ws::Message;

#[derive(Serialize)]
struct TestData {
    widget_type: String,
    widget_count: u32,
    creation_date: DateTime<Utc>,
}

fn generate_random_data() -> Vec<TestData> {
  let mut rng = rand::thread_rng();
  let mut test_data_batch = Vec::new();
  for i in 0..10 {
      test_data_batch.push(TestData {
          widget_type: String::from(format!("widget{}", i)),
          widget_count: rng.gen_range(0..100),
          creation_date: Utc::now(),
      });
  }
  return test_data_batch;
}

pub async fn main_worker(clients: Clients, mut socket: WebSocket<AutoStream>) {
    let mut pairs_data: HashMap<String, DepthStreamData> = HashMap::new();
    loop {
        // tokio::time::sleep(Duration::from_millis(2000)).await;

        let connected_client_count = clients.lock().await.len();
        if connected_client_count == 0 {
            println!("No clients connected, skip sending data");
            continue;
        }
        println!("{} connected client(s)", connected_client_count);

        let msg = socket.read_message().expect("Error reading message");
        let msg = match msg {
            tungstenite::Message::Text(s) => s,
            _ => {
                panic!("Error getting text");
            }
        };
        
        let parsed: models::DepthStreamWrapper = serde_json::from_str(&msg).expect("Can't parse");
        for i in 0..parsed.data.asks.len() {
            println!(
                "{}: {}. ask: {}, size: {}",
                parsed.stream, i, parsed.data.asks[i].price, parsed.data.asks[i].size
            );
        }
        
        // let test_data_batch = generate_random_data(); // No longer needed as data is getting fetched directly from binance
        clients.lock().await.iter().for_each(|(_, client)| {
            if let Some(sender) = &client.sender {
                let _ = sender.send(Ok(Message::text(
                    serde_json::to_string(&parsed).unwrap(),
                )));
            }
        });

    }
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

async fn process_triangle_data(
    pairs_data: &HashMap<String, DepthStreamWrapper>,
    start_pair: &str,
    mid_pair: &str,
    end_pair: &str,
    triangle: [&str; 3],
    clients: Clients,
) {
    println!(
        "processing triangle {:?}: {}->{}->{}",
        triangle, start_pair, mid_pair, end_pair
    );

    let data = (
        pairs_data.get(start_pair),
        pairs_data.get(mid_pair),
        pairs_data.get(end_pair),
    );

    let (start_pair_data, mid_pair_data, end_pair_data) = match data {
        (Some(s), Some(m), Some(e)) => (s, m, e),
        _ => {
            println!(
                "{:?} One or more of the pairs were not found, skipping",
                (start_pair, mid_pair, end_pair)
            );
            return;
        }
    };

    let mut profits: Vec<f64> = Vec::new();

    for i in 0..start_pair_data.data.asks.len() {
        let mut triangle_profit = calc_triangle_step(
            1.0,
            start_pair_data.data.asks[i].price,
            start_pair_data.data.bids[i].price,
            start_pair,
            triangle[0],
        );
        triangle_profit = calc_triangle_step(
            triangle_profit,
            mid_pair_data.data.asks[i].price,
            mid_pair_data.data.bids[i].price,
            mid_pair,
            triangle[1],
        );
        triangle_profit = calc_triangle_step(
            triangle_profit,
            end_pair_data.data.asks[i].price,
            end_pair_data.data.bids[i].price,
            end_pair,
            triangle[2],
        );

        profits.push((100.0 * triangle_profit) - 100.0);
    }

    println!("{:?} potential profits: {:?}", triangle, profits);
    let triangle_data = models::TriangleArbitrageData {
        start_pair_data: start_pair_data.clone(),
        mid_pair_data: mid_pair_data.clone(),
        end_pair_data: end_pair_data.clone(),
        profits,
        triangle: [
            triangle[0].to_string(),
            triangle[1].to_string(),
            triangle[2].to_string(),
        ],
    };

    clients.lock().await.iter().for_each(|(_, client)| {
        if let Some(sender) = &client.sender {
            let _ = sender.send(Ok(Message::text(
                serde_json::to_string(&triangle_data).unwrap(),
            )));
        }
    });
}
