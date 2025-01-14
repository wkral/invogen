use std::cmp;
use std::fmt;
use std::ops::{Add, Mul};

use chrono::{Datelike, Local, NaiveDate};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString, VariantNames};

use crate::calendar::DateBoundaries;
use crate::historical::Historical;
use crate::ledger_fmt::LedgerDisplay;

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
            Unit::Hour => Decimal::from(0),
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
            self.from.start_of_week().expect("Error in chrono utils"),
            self.until.end_of_week().expect("Error in chrono utils"),
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Service {
    pub name: String,
    pub rates: Historical<Rate>,
}

impl Service {
    pub fn new(name: String) -> Self {
        Self {
            name,
            rates: Historical::new(),
        }
    }
}

impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ", self.name)?;
        match self.rates.current() {
            None => write!(f, "(No current rate set) "),
            Some(rate) => write!(f, "{}", rate),
        }
    }
}

#[derive(
    Display,
    EnumString,
    VariantNames,
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
    Hour,
}

#[derive(
    Display,
    EnumString,
    VariantNames,
    Serialize,
    Deserialize,
    Debug,
    PartialEq,
    Clone,
    Copy,
)]
pub enum Currency {
    #[strum(serialize = "CAD $")]
    #[serde(rename = "CAD")]
    Cad,
    #[strum(serialize = "USD $")]
    #[serde(rename = "USD")]
    Usd,
    #[strum(serialize = "EUR €")]
    #[serde(rename = "EUR")]
    Eur,
}

impl LedgerDisplay for Currency {
    fn ledger_fmt(&self, buf: &mut (dyn fmt::Write)) -> fmt::Result {
        match self {
            Currency::Cad => write!(buf, "$"),
            Currency::Usd => write!(buf, "USD$"),
            Currency::Eur => write!(buf, "EUR€"),
        }
    }
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
        Self(self.0, self.1 + other.1)
    }
}

impl Mul<Decimal> for Money {
    type Output = Self;

    fn mul(self, other: Decimal) -> Self {
        Self(
            self.0,
            (self.1 * other).round_dp_with_strategy(
                2,
                RoundingStrategy::MidpointNearestEven,
            ),
        )
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{:.2}", self.0, self.1)
    }
}

impl LedgerDisplay for Money {
    fn ledger_fmt(&self, buf: &mut (dyn fmt::Write)) -> fmt::Result {
        self.0.ledger_fmt(buf)?;
        self.1.ledger_fmt(buf)
    }
}

impl LedgerDisplay for Decimal {
    fn ledger_fmt(&self, buf: &mut (dyn fmt::Write)) -> fmt::Result {
        write! {buf, "{:.2}", self}
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
pub struct TaxRate(pub String, pub Decimal);

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
pub struct InvoiceItem {
    pub name: String,
    pub rate: Rate,
    pub period: Period,
    pub quantity: Decimal,
    pub amount: Money,
}

impl InvoiceItem {
    pub fn new(name: String, rate: Rate, period: Period) -> Self {
        let quantity = period.num_units(&rate.per);
        let amount = rate.amount * quantity;
        Self {
            name,
            rate,
            period,
            quantity,
            amount,
        }
    }

    pub fn new_hourly(
        name: String,
        rate: Rate,
        period: Period,
        quantity: Decimal,
    ) -> Self {
        let amount = rate.amount * quantity;
        Self {
            name,
            rate,
            period,
            quantity,
            amount,
        }
    }
}

impl fmt::Display for InvoiceItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {}, {:.2} @ {}: {}",
            self.name, self.period, self.quantity, self.rate, self.amount
        )
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Invoice {
    pub date: NaiveDate,
    pub number: usize,
    pub items: Vec<InvoiceItem>,
    pub tax_rates: Vec<TaxRate>,
    pub paid: Option<NaiveDate>,
}

impl Invoice {
    pub fn new(
        number: usize,
        items: Vec<InvoiceItem>,
        tax_rates: Vec<TaxRate>,
    ) -> Self {
        let date = Local::now().date_naive();

        Self {
            date,
            number,
            items,
            tax_rates,
            paid: None,
        }
    }

    pub fn calculate(&self) -> InvoiceTotal {
        let subtotal = self
            .items
            .iter()
            .map(|i| i.amount)
            .reduce(|acc, x| acc + x)
            .expect("Invoice should have at least one item");
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

    pub fn overall_period(&self) -> Period {
        let (min, max) = self
            .items
            .iter()
            .map(|i| (i.period.from, i.period.until))
            .fold(
                (NaiveDate::MAX, NaiveDate::MIN),
                |(min, max), (from, until)| {
                    (cmp::min(min, from), cmp::max(max, until))
                },
            );
        Period::new(min, max)
    }
}

impl fmt::Display for Invoice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Invoice: #{}\n\
             Date: {}\n\n",
            self.number, self.date,
        )?;

        for item in self.items.iter() {
            writeln!(f, "{}", item)?;
        }

        write!(f, "\n\n{}", self.calculate())
    }
}
