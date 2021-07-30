use chrono::{DateTime, Local, NaiveDate, Utc};
use clap::Clap;
use serde::{Deserialize, Serialize};

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;

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
    #[serde(skip_serializing)]
    past_services: BTreeSet<String>,
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
            past_services: BTreeSet::new(),
        }
    }

    fn update(&mut self, update: &Update) -> Result<(), ClientError> {
        match update {
            Update::Address(addr) => self.address = addr.clone(),
            Update::Name(name) => self.name = name.clone(),
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
                self.past_services.insert(invoice.service.clone());
            }
            Update::Paid(num, when) => {
                if let Some(invoice) = self.invoices.get_mut(num) {
                    if invoice.paid.is_some() {
                        return Err(ClientError::AlreadyPaid {
                            client: self.key.clone(),
                            number: num.clone(),
                        });
                    }
                    invoice.paid = Some(*when)
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
    Name(String),
    Rate(NaiveDate, Rate),
    Invoiced(Invoice),
    Paid(usize, NaiveDate),
    Taxes(NaiveDate, Vec<TaxRate>),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
enum Change {
    Added { name: String, address: String },
    Updated(Update),
    Removed,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Event(String, DateTime<Utc>, Change);

impl Event {
    pub fn new(key: &str, change: Change) -> Self {
        Self(key.to_string(), Utc::now(), change)
    }
    pub fn new_update(key: &str, update: Update) -> Self {
        Self(key.to_string(), Utc::now(), Change::Updated(update))
    }
}

impl fmt::Display for Client {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:\n\n{}\n{}\n", self.key, self.name, self.address)
    }
}

#[derive(Clap)]
pub enum Command {
    #[clap(about = "List all clients")]
    ListClients,
    #[clap(about = "Add a new client")]
    AddClient,
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
    #[clap(about = "Change a client's address")]
    ChangeAddress {
        #[clap(about = "key name to identify the client")]
        key: String,
    },
    #[clap(about = "Change a client's name")]
    ChangeName {
        #[clap(about = "key name to identify the client")]
        key: String,
    },
    #[clap(about = "List all invoices for a client")]
    ListInvoices {
        #[clap(about = "key name to identify the client")]
        key: String,
    },
}

type Clients = BTreeMap<String, Client>;

fn apply_event(clients: &mut Clients, event: &Event) {
    let Event(ref key, _, change) = event;
    match change {
        Change::Added { name, address } => {
            clients.insert(key.clone(), Client::new(key, name, address));
        }
        Change::Updated(update) => {
            clients.get_mut(key).map(|client| client.update(update));
        }
        Change::Removed => {
            clients.remove(key);
        }
    };
}

fn from_events(events: &Vec<Event>) -> Result<Clients, ClientError> {
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

pub fn run_cmd_with_path(
    cmd: Command,
    history_path: &PathBuf,
) -> Result<(), ClientError> {
    let mut events: Vec<Event> = if history_path.as_path().exists() {
        let history_file = File::open(history_path)?;
        let reader = BufReader::new(history_file);
        serde_lexpr::from_reader(reader)?
    } else {
        Vec::new()
    };

    if let Some(event) = run_cmd(cmd, &events)? {
        events.push(event);
        let updated_path = history_path.with_extension("updated");
        let f = File::create(&updated_path)?;

        serde_lexpr::to_writer(f, &events)?;
        fs::rename(updated_path, history_path)?;
    }
    Ok(())
}

fn run_cmd(
    cmd: Command,
    events: &Vec<Event>,
) -> Result<Option<Event>, ClientError> {
    let mut clients = from_events(events)?;

    let event = match cmd {
        Command::AddClient => add_client(),
        Command::ListClients => list_clients(&clients),
        Command::Rates { key } => show_client_rates(client(&clients, &key)?),
        Command::Invoice { key } => invoice(client(&clients, &key)?),
        Command::SetTaxes { key } => set_taxes(client(&clients, &key)?),
        Command::SetRate { key } => set_rate(client(&clients, &key)?),
        Command::ChangeAddress { key } => {
            change_address(client(&clients, &key)?)
        }
        Command::ChangeName { key } => change_name(client(&clients, &key)?),
        Command::ListInvoices { key } => list_invoices(client(&clients, &key)?),
    }?;
    Ok(event.map(|e| {
        apply_event(&mut clients, &e);
        e
    }))
}

type MaybeEvent = Result<Option<Event>, ClientError>;

fn add_client() -> MaybeEvent {
    let (key, name, address) = input::client()?;
    println!("\nAdding client {}:\n\n{}\n{}", key, name, address);
    Ok(input::confirm()?
        .then(|| Event::new(&key, Change::Added { name, address })))
}

fn list_clients(clients: &Clients) -> MaybeEvent {
    for client in clients.values() {
        println!("{}", client);
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
    let service = input::service_description(&client.past_services)?;
    let rate = client.rate_as_of(period.from).ok_or(ClientError::NoRate {
        key: client.key.clone(),
        effective: period.from,
    })?;
    let taxes = client.taxes_as_of(period.from);
    let invoice =
        Invoice::new(client.next_invoice_num(), period, service, rate, taxes);

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
    Ok(input::confirm()?
        .then(|| Event::new_update(&client.key, Update::Rate(effective, rate))))
}

fn change_address(client: &Client) -> MaybeEvent {
    let address = input::address()?;

    println!("Changing address for {} to: \n\n{}", client.name, address);
    Ok(input::confirm()?
        .then(|| Event::new_update(&client.key, Update::Address(address))))
}

fn change_name(client: &Client) -> MaybeEvent {
    let name = input::name()?;
    println!(
        "Changing client {} ({}) to: \n\n{}",
        client.name, client.key, name
    );
    Ok(input::confirm()?
        .then(|| Event::new_update(&client.key, Update::Name(name))))
}

fn list_invoices(client: &Client) -> MaybeEvent {
    for i in client.invoices.values() {
        let paid = if let Some(when) = i.paid {
            format!("Paid {}", when)
        } else {
            "Unpaid".to_string()
        };
        let total = i.calculate();
        println!(
            "#{} {}, {}, {} ({})",
            i.number, i.period, i.service, total.total, paid
        )
    }
    Ok(None)
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::billing::{Currency, Money, Unit};
    use chrono::TimeZone;
    use const_format::formatcp;
    use rust_decimal::Decimal;
    use serde_lexpr::{from_str, to_string, Error};

    fn billing_rate() -> Rate {
        Rate {
            amount: Money::new(Currency::USD, Decimal::from(1000)),
            per: Unit::Month,
        }
    }

    const RATE_RAW: &str = "(amount . #(USD 1000.0)) \
         (per . Month)";

    const CLIENT_RAW: &str = "(key . \"innotech\") \
         (name . \"Innotech\") \
         (address . \"Some Place\") \
         (rates) \
         (invoices) \
         (taxes)";

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
            Utc.ymd(2021, 04, 15).and_hms(10, 30, 0),
            change,
        );
        let sexpr = to_string(&event)?;
        assert_eq!(sexpr, CLIENT_ADD_STR);
        Ok(())
    }

    const RATE_UPDATE_STR: &str = formatcp!(
        "#(\"innotech\" \"2021-04-16T09:30:00Z\" \
           (Updated Rate \"2021-04-15\" ({})))",
        RATE_RAW
    );

    #[test]
    fn serialize_update() -> Result<(), Error> {
        let update =
            Update::Rate(NaiveDate::from_ymd(2021, 04, 15), billing_rate());
        let change = Change::Updated(update);
        let event = Event(
            "innotech".to_string(),
            Utc.ymd(2021, 04, 16).and_hms(9, 30, 0),
            change,
        );
        let sexpr = to_string(&event)?;
        assert_eq!(sexpr, RATE_UPDATE_STR);
        Ok(())
    }

    const EVENTS_STR: &str =
        formatcp!("({}\n{})", CLIENT_ADD_STR, RATE_UPDATE_STR);

    #[test]
    fn client_from_events() -> Result<(), Error> {
        let events: Vec<Event> = from_str(EVENTS_STR)?;
        let client_map = from_events(&events).unwrap();

        let client = client_map.get("innotech").unwrap();
        let query_date = NaiveDate::from_ymd(2021, 04, 17);

        assert_eq!(&client.address, "Some Place");
        assert_eq!(client.rate_as_of(query_date), Some(&billing_rate()));
        Ok(())
    }

    #[test]
    fn list() -> Result<(), ClientError> {
        let history = from_str(EVENTS_STR)?;
        run_cmd(Command::ListClients, &history)?;
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
