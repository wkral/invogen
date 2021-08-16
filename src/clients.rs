use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::collections::BTreeMap;
use std::fmt;

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
                    .ok_or(ClientError::Invoice(num.clone(), NotFound))?;
                if invoice.paid.is_some() {
                    return Err(ClientError::Invoice(num.clone(), AlreadyPaid));
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
            .map(|i| i.clone())
            .flatten()
            .collect()
    }

    pub fn current_taxes(&self) -> Vec<TaxRate> {
        self.taxes
            .current()
            .into_iter()
            .map(|i| i.clone())
            .flatten()
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
            .ok_or(ClientError::Invoice(num.clone(), InvoiceError::NotFound))
    }

    pub fn service_names<'a>(&'a self) -> Vec<&'a str> {
        self.services
            .keys()
            .map(String::as_str)
            .collect::<Vec<&str>>()
    }

    pub fn service(&self, name: String) -> Option<&Service> {
        self.services.get(&name)
    }

    pub fn invoices<'a>(&'a self) -> impl Iterator<Item = &'a Invoice> {
        self.invoices.values()
    }

    pub fn unpaid_invoices<'a>(&'a self) -> impl Iterator<Item = &'a usize> {
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
        key: &String,
        client: Client,
    ) -> Result<(), ClientError> {
        self.0.insert(key.clone(), client);
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
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a Client> {
        self.0.values()
    }
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
pub enum InvoiceError {
    #[error("found after {0}")]
    OutOfSequence(usize),

    #[error("not found")]
    NotFound,

    #[error("was previously paid")]
    AlreadyPaid,
}
