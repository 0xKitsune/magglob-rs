use std::{
    collections::BTreeMap,
    rc::Weak,
    sync::{Arc, RwLock},
};

use ordered_float::{Float, OrderedFloat};
use tokio::task::JoinHandle;

use crate::{error::OrderBookError, exchanges::Exchange};

type PriceLevelTree = RwLock<BTreeMap<OrderedFloat<f64>, PriceLevel>>;
pub struct OrderBook {
    pub ticker: String,
    pub exchanges: Vec<Exchange>,
    pub bid_tree: Arc<PriceLevelTree>,
    pub ask_tree: Arc<PriceLevelTree>,
}

impl OrderBook {
    pub fn new(ticker: &str, exchanges: Vec<Exchange>) -> Self {
        OrderBook {
            ticker: String::from(ticker),
            exchanges,
            bid_tree: Arc::new(RwLock::new(BTreeMap::new())),
            ask_tree: Arc::new(RwLock::new(BTreeMap::new())),
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
                        &self.ticker,
                        order_book_depth,
                        price_level_tx.clone(),
                    )
                    .await?,
            )
        }

        let bid_tree = self.bid_tree.clone();
        let ask_tree = self.ask_tree.clone();

        handles.push(tokio::spawn(async move {
            while let Some(price_level_update) = price_level_rx.recv().await {
                update_bid_ask_trees(&bid_tree, &ask_tree, price_level_update)?;
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
                        &self.ticker,
                        order_book_depth,
                        price_level_tx.clone(),
                    )
                    .await?,
            )
        }

        let bid_tree = self.bid_tree.clone();
        let ask_tree = self.ask_tree.clone();

        handles.push(tokio::spawn(async move {
            //TODO: keep track of the bid ask spread
            while let Some(price_level_update) = price_level_rx.recv().await {
                update_bid_ask_trees(&bid_tree, &ask_tree, price_level_update)?;
            }

            Ok::<(), OrderBookError>(())
        }));

        Ok(handles)
    }

    //TODO: basically spawn a service that will listen to updates from all of the exchanges, and send an update through a channel when the orderbook has updated
    //This service will spawn a thread for each exchange that it needs to listen to and then send the update through a channel where the order book will be updated here
    //Then you can update the corresponding tx rx depending on the orderbook that is spawned
}

#[inline(always)]
fn update_bid_ask_trees(
    bid_tree: &PriceLevelTree,
    ask_tree: &PriceLevelTree,
    price_level_update: PriceLevelUpdate,
) -> Result<(), OrderBookError> {
    match price_level_update {
        PriceLevelUpdate::Bid(price_level) => {
            if price_level.quantity == 0.0 {
                bid_tree
                    .write()
                    .map_err(|_| OrderBookError::PoisonedLockOnBTreeMap)?
                    .remove(&OrderedFloat(price_level.price));
            } else {
                //Insert/update tree
                bid_tree
                    .write()
                    .map_err(|_| OrderBookError::PoisonedLockOnBTreeMap)?
                    .insert(OrderedFloat(price_level.price), price_level);
            }
        }
        PriceLevelUpdate::Ask(price_level) => {
            if price_level.quantity == 0.0 {
                ask_tree
                    .write()
                    .map_err(|_| OrderBookError::PoisonedLockOnBTreeMap)?
                    .remove(&OrderedFloat(price_level.price));
            } else {
                //Insert/update tree
                ask_tree
                    .write()
                    .map_err(|_| OrderBookError::PoisonedLockOnBTreeMap)?
                    .insert(OrderedFloat(price_level.price), price_level);
            }
        }
    }
    Ok(())
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
    use crate::{
        exchanges::{binance::Binance, Exchange, OrderBookService},
        order_book::OrderBook,
        order_book::{PriceLevel, PriceLevelUpdate},
    };

    #[tokio::test]
    async fn test_order_book_service() {
        let handles = OrderBook::new("bnbbtc", vec![Exchange::Binance])
            .spawn_order_book_service(5000, 100)
            .await
            .expect("handle error");

        let handles_0 = OrderBook::new("bnbusdt", vec![Exchange::Binance])
            .spawn_order_book_service(5000, 100)
            .await
            .expect("handle error");

        for handle in handles {
            handle.await.expect("handle error").expect("handle error");
        }
    }
}
