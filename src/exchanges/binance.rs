use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;

use crate::{error::OrderBookError, red_black_book::Order};

use super::OrderBookStream;

const BASE_ENDPOINT: &str = "wss://stream.binance.com:9443";

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

#[async_trait]
impl OrderBookStream for Binance {
    async fn spawn_order_book_stream(
        &self,
        ticker: &str,
    ) -> Result<Receiver<Order>, OrderBookError> {
        todo!()
    }
}
