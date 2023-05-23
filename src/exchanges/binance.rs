use core::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::exchanges::Exchange;
use crate::red_black_book::PriceLevelUpdate;
use crate::{
    error::OrderBookError,
    red_black_book::{OrderBook, PriceLevel},
};

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserializer,
};
use serde_derive::Deserialize;

use tokio::{
    net::TcpStream,
    sync::mpsc::{error::SendError, Receiver, Sender},
    task::JoinHandle,
};

use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::{protocol::WebSocketConfig, Message};

use super::OrderBookService;

const WS_BASE_ENDPOINT: &str = "wss://stream.binance.com:9443/ws/";
const DEPTH_SNAPSHOT_BASE_ENDPOINT: &str = "https://api.binance.com/api/v3/depth?symbol=";
const DEFAULT_DEPTH_LIMIT: &str = "10"; //TODO: 5000 should cover all per the binance docs

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
    pub event_type: OrderBookEventType,
    #[serde(rename = "E")]
    pub event_time: usize,
    #[serde(rename = "U")]
    pub first_update_id: u64, //NOTE: not positive what the largest order id from the exchange will possibly grow to, it can probably be covered by u32, but using u64 just to be safe
    #[serde(rename = "u")]
    pub final_updated_id: u64,
    #[serde(rename = "b", deserialize_with = "convert_array_items_to_f64")]
    pub bids: Vec<[f64; 2]>,
    #[serde(rename = "a", deserialize_with = "convert_array_items_to_f64")]
    pub asks: Vec<[f64; 2]>,
}

impl OrderBookUpdate {
    pub fn new(
        event_type: OrderBookEventType,
        event_time: usize,
        first_update_id: u64,
        final_updated_id: u64,
        bids: Vec<[f64; 2]>,
        asks: Vec<[f64; 2]>,
    ) -> Self {
        OrderBookUpdate {
            event_type,
            event_time,
            first_update_id,
            final_updated_id,
            bids,
            asks,
        }
    }
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
        price_level_tx: Sender<PriceLevelUpdate>,
    ) -> Result<Vec<JoinHandle<Result<(), OrderBookError>>>, OrderBookError> {
        //TODO: handle reconnects in an efficient and safe way

        let (mut order_book_rx, mut last_update_id, stream_handle) =
            self.spawn_order_book_stream(ticker).await?;

        let price_level_update_handle = tokio::spawn(async move {
            while let Some(order_book_update) = order_book_rx.recv().await {
                //TODO: FIXME: we cant do this when adding the depth snapshot, maybe we can say the depth snapshot or the initial updated id is id -1 so that the depth snapshot is counted
                if order_book_update.final_updated_id <= last_update_id {
                    continue;
                } else {
                    if order_book_update.first_update_id <= last_update_id + 1
                        && order_book_update.final_updated_id >= last_update_id + 1
                    {
                        for bid in order_book_update.bids.into_iter() {
                            price_level_tx
                                .send(PriceLevelUpdate::Bid(PriceLevel::new(
                                    bid[0],
                                    bid[1],
                                    Exchange::Binance,
                                )))
                                .await?;
                        }

                        for ask in order_book_update.asks.into_iter() {
                            price_level_tx
                                .send(PriceLevelUpdate::Ask(PriceLevel::new(
                                    ask[0],
                                    ask[1],
                                    Exchange::Binance,
                                )))
                                .await?;
                        }
                    } else {
                        return Err(BinanceError::InvalidUpdateId.into());
                    }

                    last_update_id = order_book_update.final_updated_id;
                }
            }

            Ok::<(), OrderBookError>(())
        });

        Ok(vec![stream_handle, price_level_update_handle])
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
            u64,
            JoinHandle<Result<(), OrderBookError>>,
        ),
        OrderBookError,
    > {
        let ticker = ticker.to_lowercase();
        let (tx, rx) = tokio::sync::mpsc::channel::<OrderBookUpdate>(STREAM_BUFFER);

        let atomic_last_update_id = Arc::new(AtomicU64::new(0));
        let last_update_id = atomic_last_update_id.clone();
        let stream_handle = tokio::spawn(async move {
            //Establish an infinite loop to handle a ws stream with reconnects
            loop {
                let depth_snapshot: DepthSnapshot = Binance::get_depth_snapshot(&ticker).await?;

                if last_update_id.load(Ordering::Relaxed) == 0 {
                    last_update_id.store(depth_snapshot.last_update_id, Ordering::Relaxed);
                }

                tx.send(OrderBookUpdate {
                    event_type: OrderBookEventType::DepthUpdate,
                    event_time: 0,
                    first_update_id: 0,
                    final_updated_id: depth_snapshot.last_update_id,
                    bids: depth_snapshot.bids,
                    asks: depth_snapshot.asks,
                })
                .await
                .map_err(BinanceError::from)?;

                let order_book_endpoint = WS_BASE_ENDPOINT.to_owned() + &ticker + "@depth";

                log::info!("Ws connection established");
                let (mut order_book_stream, _) =
                    tokio_tungstenite::connect_async(order_book_endpoint).await?;

                while let Some(Ok(message)) = order_book_stream.next().await {
                    match message {
                        tungstenite::Message::Text(message) => {
                            dbg!(serde_json::from_str::<OrderBookUpdate>(&message)?);

                            tx.send(serde_json::from_str(&message)?)
                                .await
                                .map_err(BinanceError::from)?;
                        }

                        tungstenite::Message::Ping(_) => {
                            log::info!("Ping received");
                            order_book_stream.send(Message::Pong(Vec::new())).await.ok();
                            log::info!("Pong sent");
                        }

                        tungstenite::Message::Close(_) => {
                            log::info!("Ws connection closed, reconnecting...");
                            break;
                        }

                        other => {
                            log::warn!("{other:?}");
                        }
                    }
                }
            }
        });

        let last_update_id = atomic_last_update_id.load(Ordering::Relaxed) - 1; //TODO:FIXME: give a detailed explantion as to why we are -1, so we dont skip our initial depth snapshot
        Ok((rx, last_update_id, stream_handle))
    }

    pub async fn get_depth_snapshot(ticker: &str) -> Result<DepthSnapshot, OrderBookError> {
        let ticker = ticker.to_uppercase();

        let depth_snapshot_endpoint =
            DEPTH_SNAPSHOT_BASE_ENDPOINT.to_owned() + &ticker + "&limit=" + DEFAULT_DEPTH_LIMIT;

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
    #[error("Invalid update id")]
    InvalidUpdateId,
}

#[cfg(test)]
mod tests {
    use crate::{
        exchanges::{binance::Binance, OrderBookService},
        red_black_book::{PriceLevel, PriceLevelUpdate},
    };

    #[tokio::test]
    async fn test_order_stream() {
        let (tx, rx) = tokio::sync::mpsc::channel::<PriceLevelUpdate>(100);
        let binance_handles = Binance::new()
            .spawn_order_book_service("bnbbtc", tx)
            .await
            .expect("handle this error");

        for handle in binance_handles {
            handle.await.expect("Waiting").expect("waiting");
        }
    }
}
