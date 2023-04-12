use std::collections::BTreeMap;

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Historical<T: Clone> {
    history: BTreeMap<NaiveDate, T>,
}

impl<T: Clone> Historical<T> {
    pub fn new() -> Self {
        Self {
            history: BTreeMap::new(),
        }
    }

    pub fn as_of(&self, date: NaiveDate) -> Option<&T> {
        self.history
            .range(..=date)
            .next_back()
            .map(|(_, item)| item)
    }

    pub fn current(&self) -> Option<&T> {
        self.as_of(Local::now().date_naive())
    }

    pub fn insert(& mut self, effective: &NaiveDate, item: &T) {
        self.history.insert(effective.clone(), item.clone());
    }
}
