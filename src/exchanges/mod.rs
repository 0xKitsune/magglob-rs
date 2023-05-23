pub mod binance;

use async_trait::async_trait;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

use crate::order_book::PriceLevelUpdate;
use crate::{error::OrderBookError, order_book::PriceLevel};

use self::binance::Binance;

#[derive(Debug)]
pub enum Exchange {
    // ByBit,
    Binance,
    // Coinbase,
    // Kraken,
    // Gemini,
}

#[async_trait]
pub trait OrderBookService {
    async fn spawn_order_book_service(
        ticker: &str,
        price_level_tx: Sender<PriceLevelUpdate>,
    ) -> Result<Vec<JoinHandle<Result<(), OrderBookError>>>, OrderBookError>;
}

impl Exchange {
    pub async fn spawn_order_book_service(
        &self,
        ticker: &str,
        price_level_tx: Sender<PriceLevelUpdate>,
    ) -> Result<Vec<JoinHandle<Result<(), OrderBookError>>>, OrderBookError> {
        match self {
            Exchange::Binance => {
                Ok(Binance::spawn_order_book_service(ticker, price_level_tx).await?)
            }
        }
    }
}
