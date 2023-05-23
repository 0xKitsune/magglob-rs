use core::fmt;

use crate::{
    error::OrderBookError,
    red_black_book::{Order, OrderBook},
};
use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use reqwest::{Response, StatusCode};
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserializer,
};
use serde_derive::Deserialize;
use serde_with::serde_as;
use serde_with::DeserializeAs;
use thiserror::Error;
use tokio::{
    net::TcpStream,
    sync::mpsc::{error::SendError, Receiver},
    task::JoinHandle,
};

use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::{protocol::WebSocketConfig, Message};

use super::OrderBookService;

const WS_BASE_ENDPOINT: &str = "wss://stream.binance.com:9443/ws/";
const DEPTH_SNAPSHOT_BASE_ENDPOINT: &str = "https://api.binance.com/api/v3/depth?symbol=";
const DEPTH_LIMIT: &str = "1000";

const STREAM_BUFFER: usize = 5000;
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

#[derive(Debug, Deserialize)]
pub struct DepthSnapshot {
    #[serde(rename = "lastUpdateId")]
    last_update_id: u64,
    #[serde(deserialize_with = "convert_array_items_to_f64")]
    bids: Vec<[f64; 2]>,
    #[serde(deserialize_with = "convert_array_items_to_f64")]
    asks: Vec<[f64; 2]>,
}

#[derive(Deserialize, Debug)]
pub struct OrderBookUpdate {
    #[serde(rename = "e")]
    event_type: OrderBookEventType,
    #[serde(rename = "E")]
    event_time: usize,
    #[serde(rename = "U")]
    first_update_id: u128,
    #[serde(rename = "u")]
    final_updated_id: u128,
    #[serde(rename = "b", deserialize_with = "convert_array_items_to_f64")]
    bids: Vec<[f64; 2]>,
    #[serde(rename = "a", deserialize_with = "convert_array_items_to_f64")]
    asks: Vec<[f64; 2]>,
}
#[derive(Deserialize, Debug)]
pub enum OrderBookEventType {
    #[serde(rename = "depthUpdate")]
    DepthUpdate,
}

#[async_trait]
impl OrderBookService for Binance {
    async fn spawn_order_book_service(
        &self,
        ticker: &str,
    ) -> Result<Receiver<Order>, OrderBookError> {
        let (order_book_rx, stream_handle) = self.spawn_order_book_stream(ticker).await?;
        let depth_snapshot = self.get_depth_snapshot(ticker).await?;

        stream_handle.await.expect("TODO: remove this from here");
        todo!("Implement this");
    }
}

impl Binance {
    pub fn new() -> Self {
        Binance {}
    }

    pub async fn spawn_order_book_stream(
        &self,
        ticker: &str,
    ) -> Result<
        (
            Receiver<OrderBookUpdate>,
            JoinHandle<Result<(), OrderBookError>>,
        ),
        OrderBookError,
    > {
        let mut ticker = ticker.to_lowercase();
        ticker.retain(|c| !c.is_whitespace());
        let order_book_endpoint = WS_BASE_ENDPOINT.to_owned() + &ticker + "@depth";

        let (mut order_book_stream, _) =
            tokio_tungstenite::connect_async(order_book_endpoint).await?;

        let (tx, rx) = tokio::sync::mpsc::channel::<OrderBookUpdate>(STREAM_BUFFER);

        let stream_handle = tokio::spawn(async move {
            while let Some(Ok(message)) = order_book_stream.next().await {
                match message {
                    tungstenite::Message::Text(message) => {
                        dbg!(serde_json::from_str::<OrderBookUpdate>(&message)?);

                        tx.send(serde_json::from_str(&message)?)
                            .await
                            .map_err(BinanceError::from)?;
                    }

                    tungstenite::Message::Ping(_) => {
                        dbg!("TODO: add logging for ping");

                        // Handle incoming ping frame
                        // Send a corresponding pong frame
                        order_book_stream.send(Message::Pong(Vec::new())).await.ok();
                    }

                    tungstenite::Message::Close(_) => {
                        dbg!("TODO: add logging for closing");

                        //TODO: Do something here to reconnect
                        break;
                    }

                    other => {
                        //TODO: do something with logging here
                        dbg!("TODO: add logging for other");
                    }
                }
            }

            Ok::<(), OrderBookError>(())
        });

        Ok((rx, stream_handle))
    }

    pub async fn get_depth_snapshot(&self, ticker: &str) -> Result<DepthSnapshot, OrderBookError> {
        let mut ticker = ticker.to_uppercase();
        ticker.retain(|c| !c.is_whitespace());

        let depth_snapshot_endpoint =
            DEPTH_SNAPSHOT_BASE_ENDPOINT.to_owned() + &ticker + "&limit=" + DEPTH_LIMIT;

        dbg!("getting here to depth", depth_snapshot_endpoint.clone());

        // Get the depth snapshot
        let depth_response = reqwest::get(depth_snapshot_endpoint).await?;

        if depth_response.status().is_success() {
            Ok(depth_response.json::<DepthSnapshot>().await?)
        } else {
            Err(OrderBookError::HTTPError(
                String::from_utf8(depth_response.bytes().await?.to_vec())
                    .expect("TODO: handle this error"),
            ))
        }
    }
}

#[derive(Debug)]
struct StringF64ArrayVisitor;

impl<'a> Visitor<'a> for StringF64ArrayVisitor {
    type Value = Vec<[f64; 2]>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a vector of two-element arrays of strings representing floats")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'a>,
    {
        let mut vec = vec![];

        while let Some(arr) = seq.next_element::<[String; 2]>()? {
            let first: f64 = arr[0].parse().map_err(de::Error::custom)?;
            let second: f64 = arr[1].parse().map_err(de::Error::custom)?;
            vec.push([first, second]);
        }

        Ok(vec)
    }
}

pub fn convert_array_items_to_f64<'a, D>(deserializer: D) -> Result<Vec<[f64; 2]>, D::Error>
where
    D: Deserializer<'a>,
{
    deserializer.deserialize_seq(StringF64ArrayVisitor)
}

#[derive(thiserror::Error, Debug)]
pub enum BinanceError {
    #[error("Order book update send error")]
    OrderBookUpdateSendError(#[from] SendError<OrderBookUpdate>),
}

#[cfg(test)]
mod tests {
    use crate::exchanges::{binance::Binance, OrderBookService};

    #[tokio::test]
    async fn test_order_stream() {
        Binance::new()
            .spawn_order_book_service("bnbbtc")
            .await
            .expect("handle this error");
    }
}
