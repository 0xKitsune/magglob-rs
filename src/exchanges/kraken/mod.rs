use core::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::exchanges::Exchange;
use crate::order_book::{self, PriceLevelUpdate};
use crate::{
    error::OrderBookError,
    order_book::{OrderBook, PriceLevel},
};

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserializer,
};
use serde_derive::{Deserialize, Serialize};

use serde_json::Value;
use tokio::{
    net::TcpStream,
    sync::mpsc::{error::SendError, Receiver, Sender},
    task::JoinHandle,
};

use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::protocol::frame::Frame;
use tungstenite::{protocol::WebSocketConfig, Message};

use super::OrderBookService;

const WS_ENDPOINT: &str = "wss://ws.kraken.com";
const STREAM_BUFFER: usize = 1000;
//Add a comment for what this is
const GET_DEPTH_SNAPSHOT: Vec<u8> = vec![];

pub struct Kraken;

#[derive(Serialize, Debug)]
pub struct SubscribeMessage {
    event: String,
    pair: Vec<String>,
    subscription: Subscription,
}

#[derive(Serialize, Debug)]
pub struct Subscription {
    name: String,
    depth: usize,
}

impl Subscription {
    pub fn new(name: &str, depth: usize) -> Self {
        Subscription {
            name: name.to_string(),
            depth,
        }
    }
}

//TODO: make an error for subscription depth not supported

impl SubscribeMessage {
    pub fn new(pair: &str, depth: usize) -> Self {
        SubscribeMessage {
            event: "subscribe".to_string(),
            pair: vec![pair.to_owned()],
            subscription: Subscription::new("book", depth),
        }
    }
}

// The best bid size and the best ask size are reported back differently between Kraken’s live trading website
//and its order book snapshot RESTful API endpoint. The difference originates from Kraken’s RESTful API having rounded these
//numbers up to three decimal places: 5.29728450 becomes 5.298 and 3.51619191 becomes 3.517. The problem is that the round happens regardless of the number: at one point we observed that a website-displayed size of 0.0001 has been
//rounded up to three decimal places and become 0.001! 10 times larger liquidity reported by the RESTful API compared to the website!!

//TODO: need to ping every 60 seconds

#[async_trait]
impl OrderBookService for Kraken {
    async fn spawn_order_book_service(
        pair: [&str; 2],
        order_book_depth: usize,
        price_level_tx: Sender<PriceLevelUpdate>,
    ) -> Result<Vec<JoinHandle<Result<(), OrderBookError>>>, OrderBookError> {
        //TODO: handle reconnects in an efficient and safe way

        let (mut order_book_rx, stream_handles) =
            Kraken::spawn_order_book_stream(pair, order_book_depth).await?;

        // let mut last_update_id = 0;
        // let price_level_update_handle = tokio::spawn(async move {
        //     while let Some(order_book_update) = order_book_rx.recv().await {
        //         if order_book_update.final_updated_id <= last_update_id {
        //             continue;
        //         } else {
        //             //TODO: make a note that the first update id will always be zero
        //             if order_book_update.first_update_id <= last_update_id + 1
        //                 && order_book_update.final_updated_id >= last_update_id + 1
        //             {
        //                 for bid in order_book_update.bids.into_iter() {
        //                     price_level_tx
        //                         .send(PriceLevelUpdate::Bid(PriceLevel::new(
        //                             bid[0],
        //                             bid[1],
        //                             Exchange::Binance,
        //                         )))
        //                         .await?;
        //                 }

        //                 for ask in order_book_update.asks.into_iter() {
        //                     price_level_tx
        //                         .send(PriceLevelUpdate::Ask(PriceLevel::new(
        //                             ask[0],
        //                             ask[1],
        //                             Exchange::Binance,
        //                         )))
        //                         .await?;
        //                 }
        //             } else {
        //                 return Err(BinanceError::InvalidUpdateId.into());
        //             }

        //             last_update_id = order_book_update.final_updated_id;
        //         }
        //     }

        //     Ok::<(), OrderBookError>(())
        // });

        let mut order_book_service_handles = vec![];
        order_book_service_handles.extend(stream_handles);
        // order_book_service_handles.push(price_level_update_handle);

        Ok(order_book_service_handles)
    }
}

#[derive(Deserialize, Debug)]
pub struct OrderBookSnapshot {
    #[serde(rename = "bs", deserialize_with = "convert_array_items_to_f64")]
    pub bids: Option<Vec<[f64; 3]>>,
    #[serde(rename = "as", deserialize_with = "convert_array_items_to_f64")]
    pub asks: Option<Vec<[f64; 3]>>,
}

#[derive(Deserialize, Debug)]
pub struct OrderBookUpdate {
    #[serde(rename = "b", deserialize_with = "convert_array_items_to_f64", default)]
    pub bids: Option<Vec<[f64; 3]>>,
    #[serde(rename = "a", deserialize_with = "convert_array_items_to_f64", default)]
    pub asks: Option<Vec<[f64; 3]>>,
}
impl Kraken {
    pub fn new() -> Self {
        Kraken {}
    }

    async fn spawn_order_book_stream(
        pair: [&str; 2],
        order_book_depth: usize,
    ) -> Result<
        (
            Receiver<OrderBookUpdate>,
            Vec<JoinHandle<Result<(), OrderBookError>>>,
        ),
        OrderBookError,
    > {
        let (ws_stream_tx, mut ws_stream_rx) = tokio::sync::mpsc::channel::<Message>(STREAM_BUFFER);

        let pair = pair.join("/").to_uppercase();
        //spawn a thread that handles the stream and buffers the results
        let stream_handle = tokio::spawn(async move {
            let ws_stream_tx = ws_stream_tx.clone();
            loop {
                let (mut order_book_stream, _) =
                    tokio_tungstenite::connect_async(WS_ENDPOINT).await?;
                log::info!("Ws connection established");

                order_book_stream
                    .send(Message::Text(
                        serde_json::to_string(&SubscribeMessage::new(&pair, order_book_depth))
                            .expect("TODO: handle this error"),
                    ))
                    .await?;

                while let Some(Ok(message)) = order_book_stream.next().await {
                    match message {
                        tungstenite::Message::Text(_) => {
                            ws_stream_tx
                                .send(message)
                                .await
                                .map_err(KrakenError::from)?;
                        }

                        tungstenite::Message::Ping(_) => {
                            log::info!("Ping received");
                            order_book_stream.send(Message::Pong(vec![])).await.ok();
                            log::info!("Pong sent");
                        }

                        tungstenite::Message::Close(_) => {
                            log::info!("Ws connection closed, reconnecting...");
                            //The kraken docs, mention to wait 5 seconds before reconnecting, should prob experiment with this
                            //to see if we can reconnect faster without penalty
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            break;
                        }

                        other => {
                            log::warn!("{other:?}");
                        }
                    }
                }
            }
        });

        let (order_book_update_tx, order_book_update_rx) =
            tokio::sync::mpsc::channel::<OrderBookUpdate>(STREAM_BUFFER);
        let order_book_update_handle = tokio::spawn(async move {
            while let Some(message) = ws_stream_rx.recv().await {
                match message {
                    tungstenite::Message::Text(message) => {
                        let val: Value = serde_json::from_str(&message)?;

                        if let Some(arr) = val.as_array() {
                            if let Some(inner_val) = arr.get(1) {
                                let x: OrderBookUpdate =
                                    serde_json::from_value(inner_val.clone()).expect("error");

                                dbg!(x);
                            }
                        }

                        // let result: OrderBookUpdate =
                        //     serde_json::from_value(json_val).expect("msg");
                        // dbg!("getting here3", &result);

                        // //Isolate the price level updates
                        // if let Value::Object(map) = &json_val[1] {
                        //     let result: OrderBookUpdate =
                        //         serde_json::from_value(Value::Object(map.clone()))
                        //             .expect("hitting issue converting to obu");
                        //     println!("{:#?}", result);
                        // }

                        // order_book_update_tx
                        //     .send(book_update)
                        //     .await
                        //     .map_err(KrakenError::from)?;
                    }

                    _ => {}
                }
            }

            Ok::<(), OrderBookError>(())
        });

        Ok((
            order_book_update_rx,
            vec![stream_handle, order_book_update_handle],
        ))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum KrakenError {
    #[error("Order book update send error")]
    OrderBookUpdateSendError(#[from] SendError<OrderBookUpdate>),
    #[error("Error when sending tungstenite message")]
    MessageSendError(#[from] SendError<Message>),
    #[error("Serde json error")]
    SerdeJsonError(#[from] serde_json::Error),
}

#[derive(Debug)]
struct StringF64ArrayVisitor;

impl<'a> Visitor<'a> for StringF64ArrayVisitor {
    type Value = Option<Vec<[f64; 3]>>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a vector of two-element arrays of strings representing floats")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'a>,
    {
        let mut vec = vec![];
        let mut is_empty = true;

        //TODO: FIXME: going to have to make two of these, one for snapshots, one for updates

        while let Some(arr) = seq.next_element::<[String; 3]>()? {
            is_empty = false; // Data is present

            let first: f64 = arr[0].parse().map_err(de::Error::custom)?;
            let second: f64 = arr[1].parse().map_err(de::Error::custom)?;
            let third: f64 = arr[2].parse().map_err(de::Error::custom)?;
            vec.push([first, second, third]);
        }

        Ok(if is_empty { None } else { Some(vec) })
    }
}

pub fn convert_array_items_to_f64<'a, D>(deserializer: D) -> Result<Option<Vec<[f64; 3]>>, D::Error>
where
    D: Deserializer<'a>,
{
    deserializer.deserialize_seq(StringF64ArrayVisitor)
}

#[cfg(test)]
mod tests {
    use crate::exchanges::kraken::Kraken;
    use crate::exchanges::OrderBookService;
    use crate::order_book::PriceLevelUpdate;

    #[tokio::test]
    async fn test_kraken_ws_stream() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<PriceLevelUpdate>(5000);

        let binance_handles = Kraken::spawn_order_book_service(["eth", "usdt"], 10, tx)
            .await
            .expect("handle this error");

        while let Some(price_level_update) = rx.recv().await {
            dbg!(price_level_update);
        }

        for handle in binance_handles {
            handle.await.expect("Waiting").expect("waiting");
        }
    }
}
