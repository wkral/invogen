use std::cmp;
use std::path::PathBuf;

use crate::billing::{Invoice, InvoiceItem, TaxRate, Unit};
use crate::cli::{Addable, Command, InvoiceView, Listable, Setable, Showable};
use crate::clients::{
    self, Change, Client, ClientError, Clients, Event, Update,
};
use crate::input;
use crate::ledger_fmt::ledger_fmt;
use crate::templates;

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use thiserror::Error;

pub fn run_cmd_with_path(
    cmd: Command,
    history_path: &PathBuf,
) -> Result<(), RunError> {
    let mut events = clients::events_from_file(history_path)?;

    if let Some(event) = run_cmd(cmd, &events)? {
        events.push(event);
        clients::events_to_file(history_path, &events)?;
    }
    Ok(())
}

type MaybeEvent = Result<Option<Event>, RunError>;

fn run_cmd(cmd: Command, events: &[Event]) -> MaybeEvent {
    let mut clients = Clients::from_events(events)?;

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
        clients.apply_event(&event)?;
        Ok(Some(event))
    } else {
        Ok(None)
    }
}

fn run_listings(clients: &Clients, listing: Listable) -> MaybeEvent {
    match listing {
        Listable::Clients => list_clients(clients),
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
    let mut start = NaiveDate::MAX;
    loop {
        let period = input::period(client.billed_until())?;
        let name = input::service_select(client.service_names())?;
        let rate = client
            .service(name.clone())
            .and_then(|s| s.rates.as_of(period.from))
            .ok_or(ClientError::NoRate(client.key.clone(), period.from))?;
        let item = if rate.per == Unit::Hour {
            let quantity = input::num_hours()?;
            InvoiceItem::new_hourly(name, rate.clone(), period, quantity)
        } else {
            InvoiceItem::new(name, rate.clone(), period)
        };
        start = cmp::min(start, item.period.from);
        items.push(item);

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
        Event::new_update(&client.key, Update::Paid(invoice.number, when))
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
        ledger_fmt(total.subtotal),
    ));

    for (TaxRate(name, _), amount) in total.taxes.iter() {
        items
            .push((format!("assets:receivable:{}", name), ledger_fmt(*amount)));
    }
    items.push((
        format!("revenues:clients:{}", client.name),
        ledger_fmt(total.total * Decimal::from(-1)),
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
    #[error("Error processing event history: {source}")]
    Event {
        #[from]
        source: clients::EventError,
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
    use crate::clients::tests::EVENTS_STR;
    use serde_lexpr::from_str;

    #[test]
    fn list() -> Result<(), RunError> {
        let history: Vec<Event> = from_str(EVENTS_STR).unwrap();
        run_cmd(
            Command::List {
                listing: Listable::Clients,
            },
            &history,
        )?;
        Ok(())
    }
}
