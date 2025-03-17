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
    loop {
        tokio::time::sleep(Duration::from_millis(2000)).await;

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