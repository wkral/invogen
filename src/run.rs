use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::PathBuf;

use crate::billing::Invoice;
use crate::clients::{Client, ClientError, Clients, Update};
use crate::input;

use chrono::{DateTime, Utc};
use clap::Clap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/* Argument Stucture
 *
 * list-clients
 * add-client
 * client <key> {
 *  | invoice
 *  | show (all | invoices | rates | taxes)
 *  | set (rate | taxes | address | name)
 *  | remove )
 * invoice <client-key> <num> (show
 *                             | paid
 *                             | posting
 *                             | markdown)
 */

#[derive(Clap)]
pub enum Command {
    #[clap(about = "List all clients")]
    ListClients,
    #[clap(about = "Add a new client")]
    AddClient,

    #[clap(about = "Work with a specific client")]
    Client {
        #[clap(about = "key name to identify the client")]
        key: String,
        #[clap(subcommand)]
        subcommand: ClientSubCmd,
    },

    #[clap(about = "Work with a specific invoice")]
    Invoice {
        #[clap(about = "key name to identify the client")]
        client_key: String,
        #[clap(about = "Invoice number with respect to the client")]
        num: usize,
        #[clap(subcommand)]
        subcommand: InvoiceSubCmd,
    },
}

#[derive(Clap)]
pub enum ClientSubCmd {
    #[clap(about = "Create a new invoice for a client")]
    Invoice,
    #[clap(about = "Show a client or specific details")]
    Show {
        #[clap(subcommand)]
        property: ClientShowSubCmd,
    },
    #[clap(about = "Set or change aspects of a client")]
    Set {
        #[clap(subcommand)]
        property: ClientSetSubCmd,
    },
    #[clap(about = "Remove the client")]
    Remove,
}

#[derive(Clap)]
pub enum ClientShowSubCmd {
    Rates,
    #[clap(about = "List all invoices for a client")]
    Invoices,
}

#[derive(Clap)]
pub enum ClientSetSubCmd {
    #[clap(about = "Set the billing rate for a client")]
    Rate,
    #[clap(about = "Set the tax rate(s) for a client")]
    Taxes,
    #[clap(about = "Change a client's address")]
    Address,
    #[clap(about = "Change a client's name")]
    Name,
}

#[derive(Clap)]
pub enum InvoiceSubCmd {
    #[clap(about = "Show the invoice")]
    Show,
    #[clap(about = "Mark the invoice paid")]
    Paid,
    #[clap(
        about = "Output a posting of the invoice and payment in ledger format"
    )]
    Posting,
    #[clap(about = "Output in markdown for generating a PDF")]
    Markdown,
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
        Command::AddClient => add_client(),
        Command::ListClients => list_clients(&clients),
        Command::Client { key, subcommand } => {
            let client = clients.get(&key)?;
            run_client_cmd(&client, subcommand)
        }
        Command::Invoice {
            client_key,
            num,
            subcommand,
        } => {
            let invoice = clients.get(&client_key)?.invoice(&num)?;
            run_invoice_cmd(invoice, subcommand)
        }
    }? {
        apply_event(&mut clients, &event)?;
        Ok(Some(event))
    } else {
        Ok(None)
    }
}

fn run_client_cmd(client: &Client, cmd: ClientSubCmd) -> MaybeEvent {
    match cmd {
        ClientSubCmd::Invoice => invoice(client),
        ClientSubCmd::Show { property } => match property {
            ClientShowSubCmd::Rates => show_client_rates(client),
            ClientShowSubCmd::Invoices => list_invoices(client),
        },
        ClientSubCmd::Set { property } => match property {
            ClientSetSubCmd::Taxes => set_taxes(client),
            ClientSetSubCmd::Rate => set_rate(client),
            ClientSetSubCmd::Name => change_name(client),
            ClientSetSubCmd::Address => change_address(client),
        },
        ClientSubCmd::Remove => Ok(None), // TODO impl
    }
}

fn run_invoice_cmd(invoice: &Invoice, cmd: InvoiceSubCmd) -> MaybeEvent {
    match cmd {
        InvoiceSubCmd::Show => Ok(None),     // TODO impl
        InvoiceSubCmd::Paid => Ok(None),     // TODO impl
        InvoiceSubCmd::Posting => Ok(None),  // TODO impl
        InvoiceSubCmd::Markdown => Ok(None), // TODO impl
    }
}

fn add_client() -> MaybeEvent {
    let (key, name, address) = input::client()?;
    println!("\nAdding client {}:\n\n{}\n{}", key, name, address);
    Ok(input::confirm()?
        .then(|| Event::new(&key, Change::Added { name, address })))
}

fn list_clients(clients: &Clients) -> MaybeEvent {
    for client in clients.iter() {
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
    for (effective, rate) in client.rates() {
        println!("{} effective {}", rate, effective);
    }
    Ok(None)
}

fn invoice(client: &Client) -> MaybeEvent {
    let period = input::period(client.billed_until())?;
    let service = input::service_description(client.past_services())?;
    let rate = client
        .rate_as_of(period.from)
        .ok_or(ClientError::NoRate(client.key.clone(), period.from))?;
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
    for i in client.invoices() {
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

    const CLIENT_RAW: &str = "(key . \"innotech\") \
         (name . \"Innotech\") \
         (address . \"Some Place\") \
         (rates) \
         (invoices) \
         (taxes)";

    const CLIENT_STR: &str = formatcp!("({})", CLIENT_RAW);

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
    fn client_from_events() -> Result<(), RunError> {
        let events: Vec<Event> = from_str(EVENTS_STR)?;
        let clients = from_events(&events).unwrap();

        let client = clients.get(&"innotech".to_string())?;
        let query_date = NaiveDate::from_ymd(2021, 04, 17);

        assert_eq!(&client.address, "Some Place");
        assert_eq!(client.rate_as_of(query_date), Some(&billing_rate()));
        Ok(())
    }

    #[test]
    fn list() -> Result<(), RunError> {
        let history = from_str(EVENTS_STR)?;
        run_cmd(Command::ListClients, &history)?;
        Ok(())
    }
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

    #[error("{source}")]
    Client {
        #[from]
        source: ClientError,
    },
}
