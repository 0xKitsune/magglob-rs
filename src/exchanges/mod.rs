pub mod binance;

use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;

use crate::{error::OrderBookError, red_black_book::Order};

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
    ) -> Result<Receiver<Order>, OrderBookError>;
}

// #[async_trait]
// pub trait MarketPriceStream {
//     async fn spawn_order_book_stream(
//         &self,
//         ticker: &str,
//     ) -> Result<Receiver<Order>, OrderBookError>;

//     //TODO: maybe reconnect
// }
