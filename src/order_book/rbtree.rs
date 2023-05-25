use ordered_float::OrderedFloat;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use super::PriceLevel;

pub struct ThreadSafeRBTree {
    tree: Arc<Mutex<BTreeMap<OrderedFloat<f64>, PriceLevel>>>,
}

impl ThreadSafeRBTree {
    pub fn new() -> Self {
        Self {
            tree: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn insert(&self, price: f64, level: PriceLevel) {
        let mut lock = self.tree.lock().unwrap();
        lock.insert(OrderedFloat(price), level);
    }

    pub fn get(&self, price: f64) -> Option<PriceLevel> {
        let lock = self.tree.lock().unwrap();
        match lock.get(&OrderedFloat(price)) {
            Some(level) => Some(level.clone()), // Make sure your `PriceLevel` type implements `Clone`
            None => None,
        }
    }

    pub fn remove(&self, price: f64) -> Option<PriceLevel> {
        let mut lock = self.tree.lock().unwrap();
        lock.remove(&OrderedFloat(price))
    }
}
