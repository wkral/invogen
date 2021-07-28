use chrono::{DateTime, Local, NaiveDate, Utc};
use clap::Clap;
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::fmt;
use std::io::{BufReader, Read, Write};

use crate::billing::{Invoice, Rate, TaxRate};
use crate::error::ClientError;
use crate::input;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Client {
    key: String,
    name: String,
    address: String,
    rates: BTreeMap<NaiveDate, Rate>,
    invoices: BTreeMap<usize, Invoice>,
    taxes: BTreeMap<NaiveDate, Vec<TaxRate>>,
}

impl Client {
    fn new(key: &str, name: &str, address: &str) -> Self {
        Self {
            key: key.to_string(),
            name: name.to_string(),
            address: address.to_string(),
            rates: BTreeMap::new(),
            invoices: BTreeMap::new(),
            taxes: BTreeMap::new(),
        }
    }

    fn update(&mut self, update: &Update) -> Result<(), ClientError> {
        match update {
            Update::Address(addr) => self.address = addr.clone(),
            Update::Rate(effective, rate) => {
                self.rates.insert(effective.clone(), rate.clone());
            }
            Update::Invoiced(invoice) => {
                if invoice.number != self.next_invoice_num() {
                    return Err(ClientError::InvoiceOutOfSequence {
                        current: self.invoices.len(),
                        found: invoice.number,
                    });
                }
                self.invoices.insert(invoice.number, invoice.clone());
            }
            Update::Paid(num) => {
                if let Some(invoice) = self.invoices.get_mut(num) {
                    if !invoice.mark_paid() {
                        return Err(ClientError::AlreadyPaid {
                            client: self.key.clone(),
                            number: num.clone(),
                        });
                    }
                } else {
                    return Err(ClientError::PaidOutOfSequence {
                        client: self.key.clone(),
                        number: num.clone(),
                    });
                }
            }
            Update::Taxes(effective, taxes) => {
                self.taxes.insert(effective.clone(), taxes.clone());
            }
        };
        Ok(())
    }

    fn rate_as_of(&self, date: NaiveDate) -> Option<&Rate> {
        self.rates.range(..=date).next_back().map(|(_, rate)| rate)
    }

    fn next_invoice_num(&self) -> usize {
        self.invoices.len() + 1
    }

    fn taxes_as_of(&self, date: NaiveDate) -> Vec<TaxRate> {
        self.taxes
            .range(..=date)
            .next_back()
            .map(|(_, rates)| rates.clone())
            .into_iter()
            .flatten()
            .collect()
    }

    fn current_rate(&self) -> Option<&Rate> {
        self.rate_as_of(Local::today().naive_local())
    }

    fn billed_until(&self) -> Option<NaiveDate> {
        self.invoices.values().last().map(|i| i.period.until)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
enum Update {
    Address(String),
    Rate(NaiveDate, Rate),
    Invoiced(Invoice),
    Paid(usize),
    Taxes(NaiveDate, Vec<TaxRate>),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
enum Change {
    Added { name: String, address: String },
    Updated(Update),
    Removed,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Event {
    key: String,
    timestamp: DateTime<Utc>,
    change: Change,
}

impl Event {
    pub fn new(key: &str, change: Change) -> Self {
        Self {
            key: key.to_string(),
            timestamp: Utc::now(),
            change: change,
        }
    }
    pub fn new_update(key: &str, update: Update) -> Self {
        Self {
            key: key.to_string(),
            timestamp: Utc::now(),
            change: Change::Updated(update),
        }
    }
}

impl fmt::Display for Client {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} @ {}", self.name, self.address)
    }
}

#[derive(Clap)]
pub enum Command {
    #[clap(about = "List all clients")]
    List,
    #[clap(about = "Add a new client")]
    Add,
    #[clap(about = "Show billing and tax rates for a client")]
    Rates {
        #[clap(about = "key name to identify the client")]
        key: String,
    },
    #[clap(about = "Create a new invoice for a client")]
    Invoice {
        #[clap(about = "key name to identify the client")]
        key: String,
    },
    #[clap(about = "Set the billing rate for a client")]
    SetRate {
        #[clap(about = "key name to identify the client")]
        key: String,
    },
    #[clap(about = "Set the tax rate(s) for a client")]
    SetTaxes {
        #[clap(about = "key name to identify the client")]
        key: String,
    },
}

type Clients = BTreeMap<String, Client>;

fn apply_event(clients: &mut Clients, event: &Event) {
    match &event.change {
        Change::Added { name, address } => {
            clients.insert(
                event.key.clone(),
                Client::new(&event.key, name, address),
            );
        }
        Change::Updated(update) => {
            clients
                .get_mut(&event.key)
                .map(|client| client.update(update));
        }
        Change::Removed => {
            clients.remove(&event.key);
        }
    };
}

fn from_events(events: Vec<Event>) -> Result<Clients, ClientError> {
    let mut clients = Clients::new();
    for event in events.iter() {
        apply_event(&mut clients, event);
    }
    Ok(clients)
}

fn client<'a>(
    clients: &'a Clients,
    key: &str,
) -> Result<&'a Client, ClientError> {
    clients.get(key).map_or(
        Err(ClientError::NotFound {
            key: key.to_string(),
        }),
        |c| Ok(c),
    )
}

pub fn run_cmd<T: Read + Write>(
    cmd: Command,
    history: T,
) -> Result<(), ClientError> {
    let reader = BufReader::new(history);
    let events: Vec<Event> = serde_lexpr::from_reader(reader)?;
    let mut clients = from_events(events)?;

    let event = match cmd {
        Command::Add => add_client(),
        Command::List => list_clients(&clients),
        Command::Rates { key } => show_client_rates(client(&clients, &key)?),
        Command::Invoice { key } => invoice(client(&clients, &key)?),
        Command::SetTaxes { key } => set_taxes(client(&clients, &key)?),
        Command::SetRate { key } => set_rate(client(&clients, &key)?),
    }?;
    event.map(|e| {
        println!("Adding event: {:?}", e);
        apply_event(&mut clients, &e)
    });
    Ok(())
}

type MaybeEvent = Result<Option<Event>, ClientError>;

fn add_client() -> MaybeEvent {
    let (key, name, address) = input::client()?;
    println!("\nAdding client {}:\n\n{}\n{}", key, name, address);
    Ok(input::confirm()?
        .then(|| Event::new(&key, Change::Added { name, address })))
}

fn list_clients(clients: &Clients) -> MaybeEvent {
    for (key, client) in clients.iter() {
        println!("{}: {}", key, client);
    }
    Ok(None)
}

fn show_client_rates(client: &Client) -> MaybeEvent {
    if let Some(current) = client.current_rate() {
        println!("Current Rate: {}\n", current);
    } else {
        println!("No current rate for client: {}", client.name);
    }

    println!("Historical Rates:\n");
    for (effective, rate) in client.rates.iter() {
        println!("{} effective {}", rate, effective);
    }
    Ok(None)
}

fn invoice(client: &Client) -> MaybeEvent {
    let period = input::period(client.billed_until())?;
    let rate = client.rate_as_of(period.from).ok_or(ClientError::NoRate {
        key: client.key.clone(),
        effective: period.from,
    })?;
    let taxes = client.taxes_as_of(period.from);
    let invoice = Invoice::new(client.next_invoice_num(), period, rate, taxes);

    println!("Adding invoice:\n\n{}", invoice);
    Ok(input::confirm()?
        .then(|| Event::new_update(&client.key, Update::Invoiced(invoice))))
}

fn set_taxes(client: &Client) -> MaybeEvent {
    let (taxes, effective) = input::taxes()?;

    println!("Setting taxes for {} to:", client.name);
    for tax in taxes.iter() {
        println!("{}", tax);
    }
    println!("Effective: {}", effective);
    Ok(input::confirm()?.then(|| {
        Event::new_update(&client.key, Update::Taxes(effective, taxes))
    }))
}

fn set_rate(client: &Client) -> MaybeEvent {
    let (rate, effective) = input::rate()?;

    println!("Setting billing rate for {} to: {}", client.name, rate);
    println!("Effective: {}", effective);
    Ok(input::confirm()?.then(|| {
        Event::new_update(&client.key, Update::Rate(effective, rate))
    }))
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::billing::{Currency, Unit};
    use chrono::TimeZone;
    use const_format::formatcp;
    use serde_lexpr::{from_str, to_string, Error};
    use std::io::Cursor;

    fn billing_rate() -> Rate {
        Rate {
            amount: 1000.0,
            currency: Currency::USD,
            per: Unit::Month,
        }
    }

    const RATE_RAW: &str = "(amount . 1000.0) \
        (currency . USD) \
        (per . Month)";

    const CLIENT_RAW: &str = "(key . \"innotech\") \
         (name . \"Innotech\") \
         (address . \"Some Place\") \
         (rates) \
         (invoices)";

    const CLIENT_STR: &str = formatcp!("({})", CLIENT_RAW);

    #[test]
    fn deserilize() {
        let client: Client = from_str(CLIENT_STR).unwrap();
        assert_eq!(client.name, "Innotech");
        assert_eq!(client.address, "Some Place");
    }

    #[test]
    fn serialize() -> Result<(), Error> {
        let client = Client::new("innotech", "Innotech", "Some Place");
        let sexpr = to_string(&client)?;
        assert_eq!(sexpr, CLIENT_STR);
        Ok(())
    }

    const CLIENT_ADD_STR: &str = formatcp!(
        "((key . \"innotech\") \
          (timestamp . \"2021-04-15T10:30:00Z\") \
          (change Added (name . \"Innotech\") (address . \"Some Place\")))",
    );

    #[test]
    fn serialize_event() -> Result<(), Error> {
        let change = Change::Added {
            name: "Innotech".to_string(),
            address: "Some Place".to_string(),
        };
        let event = Event {
            key: "innotech".to_string(),
            change: change,
            timestamp: Utc.ymd(2021, 04, 15).and_hms(10, 30, 0),
        };
        let sexpr = to_string(&event)?;
        assert_eq!(sexpr, CLIENT_ADD_STR);
        Ok(())
    }

    const RATE_UPDATE_STR: &str = formatcp!(
        "((key . \"innotech\") \
          (timestamp . \"2021-04-16T09:30:00Z\") \
          (change Updated Rate \"2021-04-15\" ({})))",
        RATE_RAW
    );

    #[test]
    fn serialize_update() -> Result<(), Error> {
        let update =
            Update::Rate(NaiveDate::from_ymd(2021, 04, 15), billing_rate());
        let change = Change::Updated(update);
        let event = Event {
            key: "innotech".to_string(),
            change: change,
            timestamp: Utc.ymd(2021, 04, 16).and_hms(9, 30, 0),
        };
        let sexpr = to_string(&event)?;
        assert_eq!(sexpr, RATE_UPDATE_STR);
        Ok(())
    }

    const EVENTS_STR: &str =
        formatcp!("({}\n{})", CLIENT_ADD_STR, RATE_UPDATE_STR);

    #[test]
    fn client_from_events() -> Result<(), Error> {
        let events: Vec<Event> = from_str(EVENTS_STR)?;
        let client_map = from_events(events).unwrap();

        let client = client_map.get("innotech").unwrap();
        let query_date = NaiveDate::from_ymd(2021, 04, 17);

        assert_eq!(&client.address, "Some Place");
        assert_eq!(client.rate_as_of(query_date), Some(&billing_rate()));
        Ok(())
    }

    #[test]
    fn list() -> Result<(), ClientError> {
        let history = Cursor::new(EVENTS_STR.as_bytes().to_owned());
        run_cmd(Command::List, history)?;
        Ok(())
    }
}

/*
#[cfg(test)]
mod proptests {
    use super::*;
    use chrono_utilities::naive::DateTransitions;
    use proptest::prelude::*;

    fn arb_currency() -> impl Strategy<Value = Currency> {
        prop_oneof![
            Just(Currency::CAD),
            Just(Currency::USD),
            Just(Currency::EUR),
        ]
    }

    fn arb_billing_unit() -> impl Strategy<Value = BillingUnit> {
        prop_oneof![
            Just(BillingUnit::Month),
            Just(BillingUnit::Week),
            Just(BillingUnit::Day),
            Just(BillingUnit::Hour),
        ]
    }

    prop_compose! {
        fn arb_date() (y in 1..10000i32, m in 1..13u32)
            (d in 1..NaiveDate::from_ymd(y, m, 1).last_day_of_month() + 1,
             y in Just(y),
             m in Just(m))
            -> NaiveDate {
            NaiveDate::from_ymd(y,m,d)
        }

    }

    prop_compose! {
        fn arb_billing_rate()
            (amount in any::<f32>().prop_map(f32::abs),
             currency in arb_currency(),
             per in arb_billing_unit()) -> Rate {

            Rate { amount, currency, per}
        }
    }

    prop_compose! {
        fn arb_client()
            (name in ".*",
             address in ".*") -> Client {
            Client::new(&name, &address)
        }
    }

    prop_compose! {
        fn arb_clients(max_size: usize)
            (vec in prop::collection::vec(arb_client(), 1..max_size))
        -> Vec<Client> {
            vec
        }
    }

    /*
    prop_compose! {
        /*
         * Start with added event
         * random number of events to follow
         * generate a vector of durations as time between events
         * generate a start date
         * map start date + durations to a sequence of increasing timestamps
         * generate a remove boolean
         * generate a sequence of updates len = durations - add [-remove]
         * map add + updates + removal to a sequence of events
         */
        fn client_events()
            (client in arb_client,
             key in "[a-z_]+",
             num_events in 0..100)
            (cli



    }
    */

    proptest! {
        #[test]
        fn test_client(clients in arb_clients(5)) {
            println!("{:?}", clients);
        }
    }
}*/
