use crate::{error::OrderBookError, red_black_book::Order};
use async_trait::async_trait;
use reqwest::{Response, StatusCode};
use serde_derive::Deserialize;
use thiserror::Error;
use tokio::sync::mpsc::Receiver;

use super::OrderBookStream;

const WS_BASE_ENDPOINT: &str = "wss://stream.binance.com:9443/ws/";
const DEPTH_SNAPSHOT_BASE_ENDPOINT: &str = "https://api.binance.com/api/v3/depth?symbol=";

const DEPTH_LIMIT: &str = "1000";

// Websocket Market Streams

// The base endpoint is: wss://stream.binance.com:9443 or wss://stream.binance.com:443
// Streams can be accessed either in a single raw stream or in a combined stream.
// Users can listen to multiple streams.
// Raw streams are accessed at /ws/<streamName>
// Combined streams are accessed at /stream?streams=<streamName1>/<streamName2>/<streamName3>
// Combined stream events are wrapped as follows: {"stream":"<streamName>","data":<rawPayload>}
// All symbols for streams are lowercase
// A single connection to stream.binance.com is only valid for 24 hours; expect to be disconnected at the 24 hour mark
// The websocket server will send a ping frame every 3 minutes. If the websocket server does not receive a pong frame back from the connection within a 10 minute period, the connection will be disconnected. Unsolicited pong frames are allowed.
// The base endpoint wss://data-stream.binance.com can be subscribed to receive market data messages. Users data stream is NOT available from this URL.
pub struct Binance {}

#[derive(Deserialize)]
pub struct DepthSnapshot {}

#[async_trait]
impl OrderBookStream for Binance {
    async fn spawn_order_book_stream(ticker: &str) -> Result<Receiver<Order>, OrderBookError> {
        // let mut ticker = ticker.to_uppercase();
        // ticker.retain(|c| !c.is_whitespace());

        let order_book_endpoint = WS_BASE_ENDPOINT.to_owned() + &ticker + "@depth";

        // Open the WebSocket stream
        let (mut order_book_stream, _) =
            tokio_tungstenite::connect_async(order_book_endpoint).await?;

        let depth_snapshot_endpoint =
            DEPTH_SNAPSHOT_BASE_ENDPOINT.to_owned() + &ticker + "&limit=" + DEPTH_LIMIT;
        // Get the depth snapshot
        let depth_response = reqwest::get(depth_snapshot_endpoint).await?;

        if depth_response.status().is_success() {
            dbg!(&depth_response);
            let depth_snapshot: DepthSnapshot = depth_response.json().await?;
        } else {
            return Err(OrderBookError::HTTPError(depth_response.json().await? conver this to a string?));
        }

        


        // let mut last_update_id = resp.U;

        // // Process the stream events
        // while let Some(msg) = order_book_stream.next().await {
        //     let msg = msg?;
        //     match msg {
        //         Message::Text(text) => {
        //             let event: Value = serde_json::from_str(&text)?;
        //         }

        //         Message::Ping(ping) => {
        //             order_book_stream.send(Message::Pong(ping)).await?;
        //         }

        //         Message::Close(close_msg) => {
        //             println!("Received close message: {:?}", close_msg);

        //             //TODO: reconnect
        //         }
        //         _ => (),
        //     }
        // }

        todo!()
    }
}
pub enum BinanceError {
    Unavailable,
}

mod tests {
    use crate::exchanges::{binance::Binance, OrderBookStream};

    #[tokio::test]
    async fn test_order_stream() {
        Binance::spawn_order_book_stream("BNnBTC")
            .await
            .expect("handle this error");
    }
}
