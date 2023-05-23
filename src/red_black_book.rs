use std::rc::Weak;

use crate::exchanges::Exchange;

pub struct OrderBook {
    ticker: String,
    exchanges: Vec<Exchange>,
    bid_tree: Option<OrderNode>, //TODO: will prob need an arc mutex or rwlock or something
    ask_tree: Option<OrderNode>,
}

impl OrderBook {
    pub fn new(ticker: &str, exchanges: Vec<Exchange>) -> Self {
        OrderBook {
            ticker: String::from(ticker),
            exchanges,
            bid_tree: None,
            ask_tree: None,
        }
    }

    //TODO: spawn market price service, update this name
    //Basically what this function will do is listen to all of the exchanges and send an update through a channel when the market price for this orderbook has updated
    pub async fn listen_to_market_price(&self) {
        //for exchange in exchanges, spawn a thread that will listen to the exchange

        // println!("Spawning OrderBook for {}", self.ticker);
    }

    //TODO: bid ask spread service, update this name
    //Basically what this function will do is listen to all of the exchanges and send an update through a channel when the bid ask spread price for this orderbook has updated
    pub async fn listen_to_bid_ask_spread(&self) {
        //for exchange in exchanges, spawn a thread that will listen to the exchange

        // println!("Spawning OrderBook for {}", self.ticker);
    }

    //TODO: basically spawn a service that will listen to updates from all of the exchanges, and send an update through a channel when the orderbook has updated
    //This service will spawn a thread for each exchange that it needs to listen to and then send the update through a channel where the order book will be updated here
    //Then you can update the corresponding tx rx depending on the orderbook that is spawned
}

pub struct Order {
    id: String,
    exchange: Exchange,
    price: f64,
    volume: f64,
}

struct OrderNode {
    color: Color, // Either Red or Black
    order: Order,
    left: Option<Box<OrderNode>>,
    right: Option<Box<OrderNode>>,
    parent: Option<Weak<OrderNode>>,
}

enum Color {
    Red,
    Black,
}

impl OrderNode {
    pub fn new(
        color: Color, // Either Red or Black
        order: Order,
        left: Option<Box<OrderNode>>,
        right: Option<Box<OrderNode>>,
        parent: Option<Weak<OrderNode>>,
    ) -> Self {
        OrderNode {
            color,
            order,
            left,
            right,
            parent,
        }
    }
}
