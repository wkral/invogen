use clap::{Parser, ValueHint};
use std::path::PathBuf;

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
pub struct Opts {
    #[clap(short, long, default_value="client.history",
        value_hint=ValueHint::FilePath)]
    pub file: PathBuf,

    #[clap(subcommand)]
    pub subcommand: Command,
}

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
