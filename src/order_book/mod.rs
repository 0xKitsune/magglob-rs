use std::{
    collections::BTreeMap,
    rc::Weak,
    sync::{Arc, RwLock},
};

use ordered_float::{Float, OrderedFloat};
use tokio::task::JoinHandle;

use crate::{error::OrderBookError, exchanges::Exchange};

pub struct AggregatedOrderBook<B: OrderBook + 'static> {
    pub pair: [String; 2],
    pub exchanges: Vec<Exchange>,
    pub order_book: Arc<B>,
}

pub trait OrderBook: Send + Sync {
    fn update_book(&self, price_level_update: PriceLevelUpdate) -> Result<(), OrderBookError>;
}

impl<B> AggregatedOrderBook<B>
where
    B: OrderBook,
{
    pub fn new(pair: [&str; 2], exchanges: Vec<Exchange>, order_book: Arc<B>) -> Self {
        AggregatedOrderBook {
            pair: [pair[0].to_string(), pair[1].to_string()],
            exchanges,
            order_book,
        }
    }

    //TODO: bid ask spread service, update this name
    //Basically what this function will do is listen to all of the exchanges and send an update through a channel when the bid ask spread price for this orderbook has updated
    pub async fn spawn_order_book_service(
        &self,
        order_book_depth: usize,
        price_level_buffer: usize,
    ) -> Result<Vec<JoinHandle<Result<(), OrderBookError>>>, OrderBookError> {
        let (price_level_tx, mut price_level_rx) =
            tokio::sync::mpsc::channel::<PriceLevelUpdate>(price_level_buffer);

        let mut handles = vec![];

        for exchange in self.exchanges.iter() {
            handles.extend(
                exchange
                    .spawn_order_book_service(
                        [&self.pair[0], &self.pair[1]],
                        order_book_depth,
                        price_level_tx.clone(),
                    )
                    .await?,
            )
        }

        let order_book = self.order_book.clone();

        handles.push(tokio::spawn(async move {
            while let Some(price_level_update) = price_level_rx.recv().await {
                order_book.update_book(price_level_update)?;

                // update_bid_ask_trees(&bid_tree, &ask_tree, price_level_update)?;
            }

            Ok::<(), OrderBookError>(())
        }));

        Ok(handles)
    }

    //TODO: spawn market price service, update this name
    //Basically what this function will do is listen to all of the exchanges and send an update through a channel when the market price for this orderbook has updated
    pub async fn listen_to_market_price(&self) {
        //for exchange in exchanges, spawn a thread that will listen to the exchange

        // println!("Spawning OrderBook for {}", self.ticker);
    }

    //TODO: bid ask spread service, update this name
    //Basically what this function will do is listen to all of the exchanges and send an update through a channel when the bid ask spread price for this orderbook has updated
    pub async fn listen_to_bid_ask_spread(
        &self,
        order_book_depth: usize,
        price_level_buffer: usize,
    ) -> Result<Vec<JoinHandle<Result<(), OrderBookError>>>, OrderBookError> {
        let (price_level_tx, mut price_level_rx) =
            tokio::sync::mpsc::channel::<PriceLevelUpdate>(price_level_buffer);

        let mut handles = vec![];

        for exchange in self.exchanges.iter() {
            handles.extend(
                exchange
                    .spawn_order_book_service(
                        [&self.pair[0], &self.pair[1]],
                        order_book_depth,
                        price_level_tx.clone(),
                    )
                    .await?,
            )
        }

        let order_book = self.order_book.clone();

        handles.push(tokio::spawn(async move {
            //TODO: keep track of the bid ask spread
            while let Some(price_level_update) = price_level_rx.recv().await {
                order_book.update_book(price_level_update)?;
            }

            Ok::<(), OrderBookError>(())
        }));

        Ok(handles)
    }

    //TODO: basically spawn a service that will listen to updates from all of the exchanges, and send an update through a channel when the orderbook has updated
    //This service will spawn a thread for each exchange that it needs to listen to and then send the update through a channel where the order book will be updated here
    //Then you can update the corresponding tx rx depending on the orderbook that is spawned
}

#[derive(Debug)]
pub struct PriceLevel {
    pub price: f64,
    pub quantity: f64,
    pub exchange: Exchange,
}

impl PriceLevel {
    pub fn new(price: f64, quantity: f64, exchange: Exchange) -> Self {
        PriceLevel {
            price,
            quantity,
            exchange,
        }
    }
}

#[derive(Debug)]
pub enum PriceLevelUpdate {
    Bid(PriceLevel),
    Ask(PriceLevel),
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        exchanges::{binance::Binance, Exchange, OrderBookService},
        order_book::AggregatedOrderBook,
        order_book::{PriceLevel, PriceLevelUpdate},
    };

    #[tokio::test]
    async fn test_order_book_service() {}
}
