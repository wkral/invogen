use std::fmt;
use std::ops::{Add, Mul};

use chrono::{Datelike, Local, NaiveDate};
use chrono_utilities::naive::DateTransitions;
use rust_decimal::{Decimal, RoundingStrategy};
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

    fn working_days(&self) -> Decimal {
        Decimal::from(
            self.from
                .iter_days()
                .take_while(|d| d <= &self.until)
                .filter(|d| d.weekday().num_days_from_monday() < 5)
                .count(),
        )
    }

    fn count_distinct<F: Fn(NaiveDate) -> u32>(&self, f: F) -> Decimal {
        Decimal::from(f(self.until) - f(self.from) + 1)
    }

    fn num_units(&self, unit: &Unit) -> Decimal {
        match unit {
            Unit::Month => self.num_months(),
            Unit::Week => self.num_weeks(),
            Unit::Day => self.working_days(),
        }
    }

    fn num_months(&self) -> Decimal {
        let full_period = Self::new(
            self.from.start_of_month().expect("Error in chorno-utils"),
            self.until.end_of_month().expect("Error in chorno-utils"),
        );
        Decimal::from((self.until.year() - self.from.year()) * 12)
            + (self.working_days() / full_period.working_days()
                * self.count_distinct(|d| d.month()))
    }

    fn num_weeks(&self) -> Decimal {
        let full_period = Self::new(
            self.from
                .start_of_iso8601_week()
                .expect("Error in chrono utils"),
            self.until
                .end_of_iso8601_week()
                .expect("Error in chrono utils"),
        );
        let distinct_weeks = Decimal::from(
            self.from
                .iter_weeks()
                .take_while(|d| d <= &self.until)
                .count(),
        );
        distinct_weeks * self.working_days() / full_period.working_days()
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
    Copy,
)]
pub enum Currency {
    #[strum(serialize = "CAD $")]
    CAD,
    #[strum(serialize = "USD $")]
    USD,
    #[strum(serialize = "EUR €")]
    EUR,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
pub struct Money(Currency, Decimal);

impl Money {
    pub fn new(currency: Currency, amount: Decimal) -> Self {
        Self(currency, amount)
    }
}

impl Add<Money> for Money {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0.clone(), self.1 + other.1)
    }
}

impl Mul<Decimal> for Money {
    type Output = Self;

    fn mul(self, other: Decimal) -> Self {
        Self(
            self.0.clone(),
            (self.1 * other).round_dp_with_strategy(
                2,
                RoundingStrategy::MidpointNearestEven,
            ),
        )
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.0, self.1)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Rate {
    pub amount: Money,
    pub per: Unit,
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.amount, self.per)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TaxRate(String, Decimal);

impl TaxRate {
    pub fn new(name: String, percentage: i64) -> Self {
        Self(name, Decimal::new(percentage, 2))
    }
}

impl fmt::Display for TaxRate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} @ {}%", self.0, self.1 * Decimal::from(100))
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct InvoiceTotal {
    pub subtotal: Money,
    pub taxes: Vec<(TaxRate, Money)>,
    pub total: Money,
}

impl fmt::Display for InvoiceTotal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Subtotal: {}", self.subtotal)?;
        for (tax_rate, amount) in self.taxes.iter() {
            writeln!(f, "{}: {}", tax_rate, amount)?;
        }

        write!(f, "\nTotal: {}", self.total)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Invoice {
    pub date: NaiveDate,
    pub number: usize,
    pub period: Period,
    pub service: String,
    pub rate: Rate,
    pub tax_rates: Vec<TaxRate>,
    pub paid: Option<NaiveDate>,
}

impl Invoice {
    pub fn new(
        number: usize,
        period: Period,
        service: String,
        rate: &Rate,
        tax_rates: Vec<TaxRate>,
    ) -> Self {
        let date = Local::today().naive_local();

        Self {
            date,
            number,
            period,
            service,
            rate: rate.clone(),
            tax_rates: tax_rates.clone(),
            paid: None,
        }
    }

    pub fn calculate(&self) -> InvoiceTotal {
        let subtotal = self.rate.amount * self.period.num_units(&self.rate.per);
        let taxes: Vec<(TaxRate, Money)> = self
            .tax_rates
            .iter()
            .map(|tr| (tr.clone(), subtotal * tr.1))
            .collect();
        let total = taxes.iter().fold(subtotal, |a, (_, x)| a + *x);

        InvoiceTotal {
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
             For: {}\n\
             {:.2} {}s @ {}\n\n\

             {}",
            self.number,
            self.period,
            self.service,
            self.period.num_units(&self.rate.per),
            self.rate.per,
            self.rate,
            self.calculate(),
        )
    }
}
