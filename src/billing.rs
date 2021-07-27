use std::fmt;

use chrono::{Datelike, NaiveDate};
use chrono_utilities::naive::DateTransitions;
use serde::{Deserialize, Serialize};

pub struct Period {
    pub from: NaiveDate,
    pub until: NaiveDate,
}

impl Period {
    pub fn new(from: NaiveDate, until: NaiveDate) -> Self {
        Self { from, until }
    }

    fn working_days(&self) -> u32 {
        self.from
            .iter_days()
            .take_while(|d| d <= &self.until)
            .filter(|d| d.weekday().num_days_from_monday() < 5)
            .count() as u32
    }

    fn count_distinct<F: Fn(NaiveDate) -> i32>(&self, f: F) -> i32 {
        f(self.until) - f(self.from) + 1
    }

    pub fn num_per(&self, unit: &Unit) -> f32 {
        match unit {
            Unit::Month => self.num_months(),
            Unit::Week => self.num_weeks(),
            Unit::Day => self.working_days() as f32,
            Unit::Hour { per_day } => self.working_days() as f32 * per_day,
        }
    }

    fn num_months(&self) -> f32 {
        let full_period = Self::new(
            self.from.start_of_month().expect("Error in chorno-utils"),
            self.until.end_of_month().expect("Error in chorno-utils"),
        );
        ((self.until.year() - self.from.year()) as f32 * 12.0)
            + (self.working_days() as f32 / full_period.working_days() as f32
                * self.count_distinct(|d| d.month() as i32) as f32)
    }

    fn num_weeks(&self) -> f32 {
        let full_period = Self::new(
            self.from
                .start_of_iso8601_week()
                .expect("Error in chrono utils"),
            self.until
                .end_of_iso8601_week()
                .expect("Error in chrono utils"),
        );
        let distinct_weeks = self
            .from
            .iter_weeks()
            .take_while(|d| d <= &self.until)
            .count();
        distinct_weeks as f32 * self.working_days() as f32
            / full_period.working_days() as f32
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum Unit {
    Month,
    Week,
    Day,
    Hour { per_day: f32 },
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let display = match self {
            Unit::Month => "Month",
            Unit::Week => "Week",
            Unit::Day => "Day",
            Unit::Hour { per_day: _ } => "Hour",
        };

        write!(f, "{}", display)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum Currency {
    CAD,
    USD,
    EUR,
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let symbol = match self {
            Currency::CAD => "CAD $",
            Currency::USD => "USD $",
            Currency::EUR => "€",
        };

        write!(f, "{}", symbol)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Rate {
    pub amount: f32,
    pub currency: Currency,
    pub per: Unit,
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{:.2}/{:?}", self.currency, self.amount, self.per)
    }
}
