use std::fmt;

use chrono::{Datelike, Local, NaiveDate};
use chrono_utilities::naive::DateTransitions;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString, EnumVariantNames};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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

    fn num_units(&self, unit: &Unit) -> f32 {
        match unit {
            Unit::Month => self.num_months(),
            Unit::Week => self.num_weeks(),
            Unit::Day => self.working_days() as f32,
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

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} — {}", self.from, self.until)
    }
}

#[derive(
    Display,
    EnumString,
    EnumVariantNames,
    Serialize,
    Deserialize,
    Debug,
    PartialEq,
    Clone,
)]
pub enum Unit {
    Month,
    Week,
    Day,
}

#[derive(
    Display,
    EnumString,
    EnumVariantNames,
    Serialize,
    Deserialize,
    Debug,
    PartialEq,
    Clone,
)]
pub enum Currency {
    #[strum(serialize = "CAD $")]
    CAD,
    #[strum(serialize = "USD $")]
    USD,
    #[strum(serialize = "EUR €")]
    EUR,
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TaxRate {
    pub name: String,
    pub percentage: u8,
}

impl fmt::Display for TaxRate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} @ {}%", self.name, self.percentage)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct InvoiceTotal {
    currency: Currency,
    subtotal: f32,
    taxes: Vec<(TaxRate, f32)>,
    total: f32,
}

impl fmt::Display for InvoiceTotal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Subtotal: {}{:.2}", self.currency, self.subtotal)?;
        for (tax_rate, amount) in self.taxes.iter() {
            writeln!(f, "{}: {}{:.2}", tax_rate, self.currency, amount)?;
        }

        write!(f, "\nTotal: {}{:.2}", self.currency, self.total)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Invoice {
    pub date: NaiveDate,
    pub number: usize,
    pub period: Period,
    pub rate: Rate,
    pub tax_rates: Vec<TaxRate>,
    pub paid: bool,
}

impl Invoice {
    pub fn new(
        number: usize,
        period: Period,
        rate: &Rate,
        tax_rates: Vec<TaxRate>,
    ) -> Self {
        let date = Local::today().naive_local();

        Self {
            date,
            number,
            period,
            rate: rate.clone(),
            tax_rates: tax_rates.clone(),
            paid: false,
        }
    }

    pub fn mark_paid(&mut self) -> bool {
        if self.paid {
            return false;
        }
        self.paid = true;
        true
    }

    pub fn calculate(&self) -> InvoiceTotal {
        let subtotal = self.rate.amount * self.period.num_units(&self.rate.per);
        let taxes: Vec<(TaxRate, f32)> = self
            .tax_rates
            .iter()
            .map(|tr| (tr.clone(), tr.percentage as f32 * subtotal / 100.0))
            .collect();
        let total = taxes.iter().fold(subtotal, |a, (_, x)| a + x);

        InvoiceTotal {
            currency: self.rate.currency.clone(),
            subtotal,
            taxes,
            total,
        }
    }
}

impl fmt::Display for Invoice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Invoice: #{}\n\
             Period: {}\n\
             {:.2} {}s @ {}\n\n\

             {}",
            self.number,
            self.period,
            self.period.num_units(&self.rate.per),
            self.rate.per,
            self.rate,
            self.calculate(),
        )
    }
}
