use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Seek, Write};
use std::path::PathBuf;

use chrono::{DateTime, NaiveDate, Utc};
use serde::ser::Error;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::billing::{Invoice, Rate, Service, TaxRate};
use crate::historical::Historical;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Client {
    pub key: String,
    pub name: String,
    pub address: String,
    pub services: BTreeMap<String, Service>,
    invoices: BTreeMap<usize, Invoice>,
    taxes: Historical<Vec<TaxRate>>,
}

impl Client {
    pub fn new(key: &str, name: &str, address: &str) -> Self {
        Self {
            key: key.to_string(),
            name: name.to_string(),
            address: address.to_string(),
            services: BTreeMap::new(),
            invoices: BTreeMap::new(),
            taxes: Historical::new(),
        }
    }

    pub fn update(&mut self, update: &Update) -> Result<(), ClientError> {
        use InvoiceError::*;
        match update {
            Update::Address(addr) => self.address = addr.clone(),
            Update::Name(name) => self.name = name.clone(),
            Update::ServiceRate(name, effective, rate) => {
                let service = self
                    .services
                    .entry(name.clone())
                    .or_insert(Service::new(name.clone()));
                service.rates.insert(effective, rate);
            }
            Update::Invoiced(invoice) => {
                if invoice.number != self.next_invoice_num() {
                    return Err(ClientError::Invoice(
                        invoice.number,
                        OutOfSequence(self.invoices.len()),
                    ));
                }
                self.invoices.insert(invoice.number, invoice.clone());
            }
            Update::Paid(num, when) => {
                let mut invoice = self
                    .invoices
                    .get_mut(num)
                    .ok_or(ClientError::Invoice(*num, NotFound))?;
                if invoice.paid.is_some() {
                    return Err(ClientError::Invoice(*num, AlreadyPaid));
                }
                invoice.paid = Some(*when)
            }
            Update::Taxes(effective, taxes) => {
                self.taxes.insert(effective, taxes);
            }
        };
        Ok(())
    }

    pub fn next_invoice_num(&self) -> usize {
        self.invoices.len() + 1
    }

    pub fn taxes_as_of(&self, date: NaiveDate) -> Vec<TaxRate> {
        self.taxes
            .as_of(date)
            .into_iter()
            .flatten()
            .cloned()
            .collect()
    }

    pub fn current_taxes(&self) -> Vec<TaxRate> {
        self.taxes
            .current()
            .into_iter()
            .flatten()
            .cloned()
            .collect()
    }

    pub fn billed_until(&self) -> Option<NaiveDate> {
        self.invoices
            .values()
            .last()
            .map(|i| i.overall_period().until)
    }

    pub fn invoice(&self, num: &usize) -> Result<&Invoice, ClientError> {
        self.invoices
            .get(num)
            .ok_or(ClientError::Invoice(*num, InvoiceError::NotFound))
    }

    pub fn service_names(&self) -> Vec<&str> {
        self.services
            .keys()
            .map(String::as_str)
            .collect::<Vec<&str>>()
    }

    pub fn service(&self, name: String) -> Option<&Service> {
        self.services.get(&name)
    }

    pub fn invoices(&self) -> impl Iterator<Item = &Invoice> {
        self.invoices.values()
    }

    pub fn unpaid_invoices(&self) -> impl Iterator<Item = &usize> {
        self.invoices()
            .filter(|i| i.paid.is_none())
            .map(|i| &i.number)
    }
}

impl fmt::Display for Client {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:\n\n{}\n{}\n", self.key, self.name, self.address)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Event(pub String, pub DateTime<Utc>, pub Change);

impl Event {
    pub fn new(key: &str, change: Change) -> Self {
        Self(key.to_string(), Utc::now(), change)
    }
    pub fn new_update(key: &str, update: Update) -> Self {
        Self(key.to_string(), Utc::now(), Change::Updated(update))
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum Change {
    Added { name: String, address: String },
    Updated(Update),
    Removed,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum Update {
    Address(String),
    Name(String),
    ServiceRate(String, NaiveDate, Rate),
    Invoiced(Invoice),
    Paid(usize, NaiveDate),
    Taxes(NaiveDate, Vec<TaxRate>),
}

pub struct Clients(BTreeMap<String, Client>);

impl Clients {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
    pub fn add(
        &mut self,
        key: &str,
        client: Client,
    ) -> Result<(), ClientError> {
        self.0.insert(key.to_owned(), client);
        Ok(())
    }
    pub fn get(&self, key: &String) -> Result<&Client, ClientError> {
        self.0
            .get(key)
            .ok_or(ClientError::NotFound(key.to_string()))
    }
    pub fn remove(&mut self, key: &String) -> Result<(), ClientError> {
        self.0
            .remove(key)
            .map(|_| ())
            .ok_or(ClientError::NotFound(key.to_string()))
    }
    pub fn update(
        &mut self,
        key: &String,
        update: &Update,
    ) -> Result<(), ClientError> {
        let client = self
            .0
            .get_mut(key)
            .ok_or(ClientError::NotFound(key.to_string()))?;
        client.update(update)?;
        Ok(())
    }
    pub fn iter(&self) -> impl Iterator<Item = &Client> {
        self.0.values()
    }

    pub fn from_events(events: &[Event]) -> Result<Self, ClientError> {
        let mut clients = Self::new();
        for event in events.iter() {
            clients.apply_event(event)?;
        }
        Ok(clients)
    }

    pub fn apply_event(&mut self, event: &Event) -> Result<(), ClientError> {
        let Event(ref key, _, change) = event;
        match change {
            Change::Added { name, address } => {
                self.add(key, Client::new(key, name, address))
            }
            Change::Updated(update) => self.update(key, update),
            Change::Removed => self.remove(key),
        }
    }
}

type FormatParser = fn(&mut BufReader<File>) -> Result<Vec<Event>, EventError>;

pub fn events_from_file(path: &PathBuf) -> Result<Vec<Event>, EventError> {
    if !path.as_path().exists() {
        Ok(Vec::new())
    } else {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let funcs: Vec<FormatParser> =
            vec![read_current_format, read_0_1_3_format];

        for func in &funcs {
            reader.rewind()?;
            if let Ok(events) = func(&mut reader) {
                return Ok(events);
            };
        }
        Err(EventError::from(serde_lexpr::Error::custom(
            "No existing or previous formats match the history file format",
        )))
    }
}

fn read_current_format(
    reader: &mut BufReader<File>,
) -> Result<Vec<Event>, EventError> {
    let mut events: Vec<Event> = Vec::new();
    for line in reader.lines() {
        events.push(serde_lexpr::from_str(line?.as_str())?);
    }
    Ok(events)
}

fn read_0_1_3_format(
    reader: &mut BufReader<File>,
) -> Result<Vec<Event>, EventError> {
    Ok(serde_lexpr::from_reader(reader)?)
}

pub fn events_to_file(
    path: &PathBuf,
    events: &[Event],
) -> Result<(), EventError> {
    let updated_path = path.with_extension("updated");

    let mut f = File::create(&updated_path)?;
    for event in events.iter() {
        serde_lexpr::to_writer(&mut f, &event)?;
        f.write_all(b"\n")?;
    }

    fs::rename(updated_path, path)?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Client Error: No client found for: '{0}'")]
    NotFound(String),

    #[error("Client Error: No effective rate found for: '{0}' as of {1}")]
    NoRate(String, NaiveDate),

    #[error("Invoice #{0} {1}")]
    Invoice(usize, InvoiceError),
}

#[derive(Debug, Error)]
pub enum EventError {
    #[error("IO Error: {source}")]
    Io {
        #[from]
        source: io::Error,
    },

    #[error("Error decoding history: {source}")]
    Format {
        #[from]
        source: serde_lexpr::Error,
    },
}

#[derive(Debug, Error)]
pub enum InvoiceError {
    #[error("found after {0}")]
    OutOfSequence(usize),

    #[error("not found")]
    NotFound,

    #[error("was previously paid")]
    AlreadyPaid,
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::billing::{Currency, Money, Rate, Unit};
    use chrono::{NaiveDate, TimeZone, Utc};
    use const_format::formatcp;
    use rust_decimal::Decimal;
    use serde_lexpr::{from_str, to_string, Error};

    fn billing_rate() -> Rate {
        Rate {
            amount: Money::new(Currency::Usd, Decimal::from(1000)),
            per: Unit::Month,
        }
    }

    const RATE_RAW: &str = "(amount . #(USD 1000.0)) \
         (per . Month)";

    const CLIENT_ADD_STR: &str = formatcp!(
        "#(\"innotech\" \"2021-04-15T10:30:00Z\" \
           (Added (name . \"Innotech\") (address . \"Some Place\")))",
    );

    #[test]
    fn serialize_event() -> Result<(), Error> {
        let change = Change::Added {
            name: "Innotech".to_string(),
            address: "Some Place".to_string(),
        };
        let event = Event(
            "innotech".to_string(),
            Utc.with_ymd_and_hms(2021, 04, 15, 10, 30, 0)
                .single()
                .unwrap(),
            change,
        );
        let sexpr = to_string(&event)?;
        assert_eq!(sexpr, CLIENT_ADD_STR);
        Ok(())
    }

    const RATE_UPDATE_STR: &str = formatcp!(
        "#(\"innotech\" \"2021-04-16T09:30:00Z\" \
           (Updated ServiceRate \"Stuff\" \"2021-04-15\" ({})))",
        RATE_RAW
    );

    #[test]
    fn serialize_update() -> Result<(), Error> {
        let update = Update::ServiceRate(
            "Stuff".to_string(),
            NaiveDate::from_ymd_opt(2021, 04, 15).unwrap(),
            billing_rate(),
        );
        let change = Change::Updated(update);
        let event = Event(
            "innotech".to_string(),
            Utc.with_ymd_and_hms(2021, 04, 16, 9, 30, 0)
                .single()
                .unwrap(),
            change,
        );
        let sexpr = to_string(&event)?;
        assert_eq!(sexpr, RATE_UPDATE_STR);
        Ok(())
    }

    pub const EVENTS_STR: &str =
        formatcp!("({}\n{})", CLIENT_ADD_STR, RATE_UPDATE_STR);

    #[test]
    fn client_from_events() -> Result<(), ClientError> {
        let events: Vec<Event> = from_str(EVENTS_STR).unwrap();
        let clients = Clients::from_events(&events)?;

        let client = clients.get(&"innotech".to_string())?;
        let query_date = NaiveDate::from_ymd_opt(2021, 04, 17).unwrap();
        let service = client.services.get("Stuff").unwrap();

        assert_eq!(&client.address, "Some Place");
        assert_eq!(&service.name, "Stuff");
        assert_eq!(service.rates.as_of(query_date), Some(&billing_rate()));
        Ok(())
    }
}
