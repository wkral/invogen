use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::Path;

use crate::clients;

use chrono::{DateTime, NaiveDate, Utc};
use clap::Clap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Invoice {
    client_key: String,
    date: NaiveDate,
    number: usize,
    from: NaiveDate,
    to: NaiveDate,
    periods: f32,
    subtotal: f32,
    rate: clients::BillingRate,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Event {
    client_key: String,
    timestamp: DateTime<Utc>,
    change: Change,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
enum Change {
    Invoiced(Invoice),
    Paid(usize),
}

struct BillingStatus {
    count: usize,
    paid: BTreeMap<usize, Invoice>,
    unpaid: BTreeMap<usize, Invoice>,
}

impl BillingStatus {
    fn new() -> Self {
        Self {
            count: 0,
            paid: BTreeMap::new(),
            unpaid: BTreeMap::new(),
        }
    }
}

#[derive(Clap)]
pub enum Command {
    Gen { client: String },
    Paid { client: String, number: usize },
}

#[derive(Debug, Error)]
pub enum InvoiceError {
    #[error("IO Error")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("Format Error")]
    Format {
        #[from]
        source: serde_lexpr::Error,
    },

    #[error("Invoices are out of sequence: after {current} found: {found}")]
    OutOfSequence { current: usize, found: usize },

    #[error(
        "Invoice {number} recoded as paid \
        before it exist or bas been paid"
    )]
    PaidOutOfSequence { number: usize },
}

fn load() -> Result<HashMap<String, BillingStatus>, InvoiceError> {
    let file = fs::File::open(Path::new("invoices"))?;
    let reader = io::BufReader::new(file);
    let events: Vec<Event> = serde_lexpr::from_reader(reader)?;
    let status_map = from_events(events)?;
    Ok(status_map)
}

fn from_events(
    events: Vec<Event>,
) -> Result<HashMap<String, BillingStatus>, InvoiceError> {
    let mut statuses = HashMap::new();
    for event in events.iter() {
        match &event.change {
            Change::Invoiced(invoice) => {
                let status = statuses
                    .entry(event.client_key.clone())
                    .or_insert(BillingStatus::new());
                if invoice.number - status.count != 1 {
                    return Err(InvoiceError::OutOfSequence {
                        current: status.count,
                        found: invoice.number,
                    });
                }
                status.unpaid.insert(invoice.number, invoice.clone());
            }
            Change::Paid(number) => {
                if let Some(status) = statuses.get_mut(&event.client_key) {
                    if let Some(invoice) = status.unpaid.remove(number) {
                        status.paid.insert(number.clone(), invoice);
                    } else {
                        return Err(InvoiceError::PaidOutOfSequence {
                            number: number.clone(),
                        });
                    }
                } else {
                    return Err(InvoiceError::PaidOutOfSequence {
                        number: number.clone(),
                    });
                }
            }
        }
    }
    Ok(statuses)
}

fn run_cmd(cmd: Command) -> Result<(), InvoiceError> {
    let status = load()?;
    match cmd {
        Command::Gen { client } => {}
        Command::Paid { client, number } => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {}
