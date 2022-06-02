use std::cmp;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::PathBuf;

use crate::billing::{Invoice, InvoiceItem, TaxRate};
use crate::clients::{Client, ClientError, Clients, Update};
use crate::input;
use crate::templates;

use chrono::naive::MAX_DATE;
use chrono::{DateTime, Datelike, Utc};
use clap::Parser;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/* Argument Stucture
 *
 * list [clients | invoices <client> | services <client>]
 * add [client | service <client>]
 * show <client> ( taxes |
 *      invoice <num> (posting | payment | markdown)
 * set <client> [rate | taxes | address | name ]
 * invoice <client>
 * mark-paid <client> <number>
 * remove <client>
 */

#[derive(Parser)]
pub enum Command {
    /// List clients, services, or invoices
    List {
        #[clap(subcommand)]
        listing: Listable,
    },

    /// Add a new client or service
    Add {
        #[clap(subcommand)]
        property: Addable,
    },

    /// Show clients and invoices
    Show {
        /// key name to identify the client
        client: String,
        #[clap(subcommand)]
        property: Option<Showable>,
    },

    /// Set properties of clients and services
    Set {
        /// key name to identify the client
        client: String,
        #[clap(subcommand)]
        property: Setable,
    },

    /// Generate a new invoice for a client
    Invoice {
        /// key name to identify the client
        client: String,
    },

    /// Record an invoice as paid
    MarkPaid {
        /// key name to identify the client
        client: String,
        /// Invoice number to show
        number: usize,
    },

    /// Remove a client, all history will be maintained
    Remove {
        /// key name to identify the client
        client: String,
    },
}

#[derive(Parser)]
pub enum Addable {
    /// Add a new client
    Client,
    /// Add a service with billing rate for a client
    Service {
        /// key name to identify the client
        client: String,
    },
}

#[derive(Parser)]
pub enum Listable {
    /// List current client
    Clients,
    /// List invoices for a client
    Invoices {
        /// key name to identify the client
        client: String,
    },
    /// List services billable to a client
    Services {
        /// key name to identify the client
        client: String,
    },
}

#[derive(Parser)]
pub enum Showable {
    /// Show taxes applied to client invoices
    Taxes,
    /// Show an invoice or in specialized formats
    Invoice {
        /// Invoice number to show
        number: usize,
        #[clap(subcommand)]
        view: Option<InvoiceView>,
    },
}

#[derive(Parser)]
pub enum Setable {
    /// Set the billing rate for a client service
    Rate,
    /// Set the tax rate(s) for a client
    Taxes,
    /// Change a client's address
    Address,
    /// Change a client's name
    Name,
}

#[derive(Parser)]
pub enum InvoiceView {
    /// Invoice in ledger format
    Posting,
    /// Payment in ledger format
    Payment,
    /// Latex format of the invoice
    Latex,
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
enum Change {
    Added { name: String, address: String },
    Updated(Update),
    Removed,
}

fn apply_event(
    clients: &mut Clients,
    event: &Event,
) -> Result<(), ClientError> {
    let Event(ref key, _, change) = event;
    match change {
        Change::Added { name, address } => {
            clients.add(key, Client::new(key, name, address))
        }
        Change::Updated(update) => clients.update(key, update),
        Change::Removed => clients.remove(key),
    }
}

fn from_events(events: &Vec<Event>) -> Result<Clients, ClientError> {
    let mut clients = Clients::new();
    for event in events.iter() {
        apply_event(&mut clients, event)?;
    }
    Ok(clients)
}

pub fn run_cmd_with_path(
    cmd: Command,
    history_path: &PathBuf,
) -> Result<(), RunError> {
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

type MaybeEvent = Result<Option<Event>, RunError>;

fn run_cmd(cmd: Command, events: &Vec<Event>) -> MaybeEvent {
    let mut clients = from_events(events)?;

    if let Some(event) = match cmd {
        Command::Add { property } => match property {
            Addable::Client => add_client(),
            Addable::Service { client } => add_service(clients.get(&client)?),
        },
        Command::List { listing } => run_listings(&clients, listing),
        Command::Invoice { client } => invoice(clients.get(&client)?),
        Command::Show { client, property } => {
            run_show(clients.get(&client)?, property)
        }
        Command::Set { client, property } => {
            let client = clients.get(&client)?;
            match property {
                Setable::Taxes => set_taxes(client),
                Setable::Rate => set_rate(client),
                Setable::Name => change_name(client),
                Setable::Address => change_address(client),
            }
        }
        Command::MarkPaid { client, number } => {
            let client = clients.get(&client)?;
            let invoice = client.invoice(&number)?;
            mark_paid(invoice, client)
        }
        Command::Remove { client: _ } => Ok(None), // TODO impl
    }? {
        apply_event(&mut clients, &event)?;
        Ok(Some(event))
    } else {
        Ok(None)
    }
}

fn run_listings(clients: &Clients, listing: Listable) -> MaybeEvent {
    match listing {
        Listable::Clients => list_clients(&clients),
        Listable::Invoices { client } => list_invoices(clients.get(&client)?),
        Listable::Services { client } => list_services(clients.get(&client)?),
    }
}

fn run_show(client: &Client, property: Option<Showable>) -> MaybeEvent {
    match property {
        None => show_client(client),
        Some(prop) => match prop {
            Showable::Taxes => Ok(None), // TODO show_client_taxes(client),
            Showable::Invoice { number, view } => {
                let invoice = client.invoice(&number)?;
                run_show_invoice(invoice, client, view)
            }
        },
    }
}

fn run_show_invoice(
    invoice: &Invoice,
    client: &Client,
    view: Option<InvoiceView>,
) -> MaybeEvent {
    match view {
        None => show_invoice(invoice),
        Some(view) => match view {
            InvoiceView::Payment => Ok(None), // TODO invoice_payment_posting(invoice, client),
            InvoiceView::Posting => invoice_posting(invoice, client),
            InvoiceView::Latex => invoice_tex(invoice, client),
        },
    }
}

fn add_client() -> MaybeEvent {
    let (key, name, address) = input::client()?;
    println!("\nAdding client {}:\n\n{}\n{}", key, name, address);
    Ok(input::confirm()?
        .then(|| Event::new(&key, Change::Added { name, address })))
}

fn add_service(client: &Client) -> MaybeEvent {
    let (name, rate, effective) = input::service()?;
    println!("\nAdding service {} for client {}", name, client.name);
    println!("Billing at: {}", rate);
    println!("Effective: {}", effective);
    Ok(input::confirm()?.then(|| {
        Event::new_update(
            &client.key,
            Update::ServiceRate(name, effective, rate),
        )
    }))
}

fn list_clients(clients: &Clients) -> MaybeEvent {
    for client in clients.iter() {
        println!("{}", client);
    }
    Ok(None)
}

fn show_client(client: &Client) -> MaybeEvent {
    println!("{}", client);

    list_services(client)?;

    for tax in client.current_taxes().iter() {
        println!("Tax: {}", tax);
    }

    if let Some(date) = client.billed_until() {
        println!("Billed Until: {}", date);
    }

    print!("Outstanding invoices:");
    for num in client.unpaid_invoices() {
        print!(" #{}", num);
    }

    Ok(None)
}

fn invoice(client: &Client) -> MaybeEvent {
    let mut items: Vec<InvoiceItem> = Vec::new();
    let mut start = MAX_DATE;
    loop {
        let period = input::period(client.billed_until())?;
        let name = input::service_select(client.service_names())?;
        let rate = client
            .service(name.clone())
            .map(|s| s.rates.as_of(period.from))
            .flatten()
            .ok_or(ClientError::NoRate(client.key.clone(), period.from))?;
        start = cmp::min(start, period.from);
        items.push(InvoiceItem::new(name.clone(), rate.clone(), period));

        if !input::another()? {
            break;
        }
    }
    let taxes = client.taxes_as_of(start);
    let invoice = Invoice::new(client.next_invoice_num(), items, taxes);

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
    let service = input::service_select(client.service_names())?;
    let (rate, effective) = input::rate()?;

    println!(
        "Setting billing rate for {}, for {} to: {}",
        service, client.name, rate
    );
    println!("Effective: {}", effective);
    Ok(input::confirm()?.then(|| {
        Event::new_update(
            &client.key,
            Update::ServiceRate(service, effective, rate),
        )
    }))
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
    for i in client.invoices() {
        let paid = if let Some(when) = i.paid {
            format!("Paid {}", when)
        } else {
            "Unpaid".to_string()
        };
        let total = i.calculate();
        println!("#{} {}, {} ({})", i.number, i.date, total.total, paid)
    }
    Ok(None)
}

fn list_services(client: &Client) -> MaybeEvent {
    for service in client.services.values() {
        println!("{}", service);
    }
    Ok(None)
}

fn show_invoice(invoice: &Invoice) -> MaybeEvent {
    println!("{}", invoice);
    Ok(None)
}

fn mark_paid(invoice: &Invoice, client: &Client) -> MaybeEvent {
    let when = input::paid_date(invoice.date)?;

    println!("Marking invoice #{} as paid on {}", invoice.number, when);
    Ok(input::confirm()?.then(|| {
        Event::new_update(
            &client.key,
            Update::Paid(invoice.number.clone(), when),
        )
    }))
}

fn invoice_posting(invoice: &Invoice, client: &Client) -> MaybeEvent {
    let total = invoice.calculate();
    let period = invoice.overall_period();
    let start = period.from.format("%b %-d");
    let end =
        period
            .until
            .format(if period.from.month() == period.until.month() {
                "%-d"
            } else {
                "%b %-d"
            });

    let mut items = Vec::new();

    items.push((
        format!("assets:receivable:{}", client.name),
        format!("{}", total.subtotal),
    ));

    for (TaxRate(name, _), amount) in total.taxes.iter() {
        items.push((
            format!("assets:receivable:{}", name),
            format!("{}", amount),
        ));
    }
    items.push((
        format!("revenues:clients:{}", client.name),
        format!("{}", total.total * Decimal::from(-1)),
    ));

    println!(
        "{} {} invoice  ; {} - {}",
        invoice.date, client.name, start, end
    );

    let max_len = items
        .iter()
        .map(|(a, b)| a.len() + b.len())
        .fold(0, |max, x| if max > x { max } else { x });

    for (account, amount) in items.iter() {
        let padding = max_len - account.len() + 4;
        println!("    {0}{1:>2$}", account, amount, padding);
    }

    Ok(None)
}

fn invoice_tex(invoice: &Invoice, client: &Client) -> MaybeEvent {
    templates::invoice(invoice, client)?;
    Ok(None)
}

#[derive(Debug, Error)]
pub enum RunError {
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

    #[error("Input Error: {source}")]
    Input {
        #[from]
        source: inquire::error::InquireError,
    },

    #[error("Render Error: {source}")]
    Render {
        #[from]
        source: askama::Error,
    },

    #[error("{source}")]
    Client {
        #[from]
        source: ClientError,
    },
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::billing::{Currency, Money, Rate, Unit};
    use chrono::{NaiveDate, TimeZone};
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
           (Updated ServiceRate \"Stuff\" \"2021-04-15\" ({})))",
        RATE_RAW
    );

    #[test]
    fn serialize_update() -> Result<(), Error> {
        let update = Update::ServiceRate(
            "Stuff".to_string(),
            NaiveDate::from_ymd(2021, 04, 15),
            billing_rate(),
        );
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
    fn client_from_events() -> Result<(), RunError> {
        let events: Vec<Event> = from_str(EVENTS_STR)?;
        let clients = from_events(&events).unwrap();

        let client = clients.get(&"innotech".to_string())?;
        let query_date = NaiveDate::from_ymd(2021, 04, 17);
        let service = client.services.get("Stuff").unwrap();

        assert_eq!(&client.address, "Some Place");
        assert_eq!(&service.name, "Stuff");
        assert_eq!(service.rates.as_of(query_date), Some(&billing_rate()));
        Ok(())
    }

    #[test]
    fn list() -> Result<(), RunError> {
        let history = from_str(EVENTS_STR)?;
        run_cmd(
            Command::List {
                listing: Listable::Clients,
            },
            &history,
        )?;
        Ok(())
    }
}
