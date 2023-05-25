use core::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub mod error;
mod stream;

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
use serde_derive::Deserialize;

use tokio::{
    net::TcpStream,
    sync::mpsc::{error::SendError, Receiver, Sender},
    task::JoinHandle,
};

use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::protocol::frame::Frame;
use tungstenite::{protocol::WebSocketConfig, Message};

use self::error::BinanceError;

use super::OrderBookService;

pub struct Binance;

impl Binance {
    pub fn new() -> Self {
        Binance {}
    }
}

#[async_trait]
impl OrderBookService for Binance {
    async fn spawn_order_book_service(
        pair: [&str; 2],
        order_book_depth: usize,
        price_level_tx: Sender<PriceLevelUpdate>,
    ) -> Result<Vec<JoinHandle<Result<(), OrderBookError>>>, OrderBookError> {
        let (mut order_book_rx, stream_handles) =
            Binance::spawn_order_book_stream(pair, order_book_depth).await?;

        let mut last_update_id = 0;
        let price_level_update_handle = tokio::spawn(async move {
            while let Some(order_book_update) = order_book_rx.recv().await {
                if order_book_update.final_updated_id <= last_update_id {
                    continue;
                } else {
                    //TODO: make a note that the first update id will always be zero
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

        let mut order_book_service_handles = vec![];
        order_book_service_handles.extend(stream_handles);
        order_book_service_handles.push(price_level_update_handle);

        Ok(order_book_service_handles)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        exchanges::{binance::Binance, OrderBookService},
        order_book::{PriceLevel, PriceLevelUpdate},
    };

    #[tokio::test]
    async fn test_binance_ws_stream() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<PriceLevelUpdate>(5000);
        let binance_handles = Binance::spawn_order_book_service(["bnb", "btc"], 5000, tx)
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
