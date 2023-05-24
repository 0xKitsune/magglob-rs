pub mod binance;
pub mod bybit;
pub mod coinbase;
pub mod crypto_dot_com;
pub mod kraken;

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
        pair: [&str; 2],
        order_book_depth: usize,
        price_level_tx: Sender<PriceLevelUpdate>,
    ) -> Result<Vec<JoinHandle<Result<(), OrderBookError>>>, OrderBookError>;
}

impl Exchange {
    pub async fn spawn_order_book_service(
        &self,
        pair: [&str; 2],
        order_book_depth: usize,
        price_level_tx: Sender<PriceLevelUpdate>,
    ) -> Result<Vec<JoinHandle<Result<(), OrderBookError>>>, OrderBookError> {
        match self {
            Exchange::Binance => {
                Ok(
                    Binance::spawn_order_book_service(pair, order_book_depth, price_level_tx)
                        .await?,
                )
            }
        }
    }
}
