use std::{collections::BTreeMap, sync::RwLock};

use ordered_float::OrderedFloat;

use crate::error::OrderBookError;

use super::{OrderBook, PriceLevel, PriceLevelUpdate};

pub struct BTreeMapOrderBook {
    bid_tree: RwLock<BTreeMap<OrderedFloat<f64>, PriceLevel>>,
    ask_tree: RwLock<BTreeMap<OrderedFloat<f64>, PriceLevel>>,
}

impl BTreeMapOrderBook {
    pub fn new() -> Self {
        BTreeMapOrderBook {
            bid_tree: RwLock::new(BTreeMap::new()),
            ask_tree: RwLock::new(BTreeMap::new()),
        }
    }
}

impl OrderBook for BTreeMapOrderBook {
    #[inline(always)]
    fn update_book(&self, price_level_update: PriceLevelUpdate) -> Result<(), OrderBookError> {
        match price_level_update {
            PriceLevelUpdate::Bid(price_level) => {
                if price_level.quantity == 0.0 {
                    self.bid_tree
                        .write()
                        .map_err(|_| OrderBookError::PoisonedLock)?
                        .remove(&OrderedFloat(price_level.price));
                } else {
                    self.bid_tree
                        .write()
                        .map_err(|_| OrderBookError::PoisonedLock)?
                        .insert(OrderedFloat(price_level.price), price_level);
                }
            }
            PriceLevelUpdate::Ask(price_level) => {
                if price_level.quantity == 0.0 {
                    self.ask_tree
                        .write()
                        .map_err(|_| OrderBookError::PoisonedLock)?
                        .remove(&OrderedFloat(price_level.price));
                } else {
                    self.ask_tree
                        .write()
                        .map_err(|_| OrderBookError::PoisonedLock)?
                        .insert(OrderedFloat(price_level.price), price_level);
                }
            }
        }
        Ok(())
    }
}
