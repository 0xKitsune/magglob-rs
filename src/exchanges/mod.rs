pub mod binance;

use async_trait::async_trait;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

use crate::red_black_book::PriceLevelUpdate;
use crate::{error::OrderBookError, red_black_book::PriceLevel};

#[derive(Debug)]
pub enum Exchange {
    ByBit,
    Binance,
    Coinbase,
    Kraken,
    Gemini,
}

#[async_trait]
pub trait OrderBookService {
    async fn spawn_order_book_service(
        &self,
        ticker: &str,
        price_level_tx: Sender<PriceLevelUpdate>,
    ) -> Result<Vec<JoinHandle<Result<(), OrderBookError>>>, OrderBookError>;
}
