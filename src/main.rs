/*
 * File with a list of clients
 *
 * 0.1 Requirements:
 * Add and store a client
 * - set the billing rate
 * - set name and address
 * - set the period a rate is active
 * Generate an invoice
 * - back dated invoice possible
 * - Record the invoice
 * - Construct the markdown to generate an invoice pdf
 * - Add an hledger entry for the invoice
 * - Regenerate PDF of an existing invoice
 *
 * Client has:
 *  - Name
 *  - Address
 *  - Number of invoices genreated,
 *  - billing period/unit,
 *  - unit rate
 *  - currency
 *  - billed until date
 *
 * To generate an invoice:
 *  - Select client
 *  - Select number of periods to bill
 *      - Default: number of periods from last billed date until now
 *  - Confirm billing period
 *      - Default: billed until date - date of end of last valid period
 *  - Calculate subtotal for invoice
 *  - Calculate tax rate for invoice
 *  - Caclulate total for invoice
 *  - Create entry in ledger file
 *  - Create generate latex invoice source
 *  - Update client properties changed by invoice
 *  - Add ledger file and invoice source to git index
 *
 *  Client data stored in TOML?
 */

mod billing;
mod clients;
mod error;
mod input;

use clap::Clap;
use std::io;

#[derive(Clap)]
struct Opts {
    #[clap(subcommand)]
    subcommand: clients::Command,
}

fn main() {
    let opts = Opts::parse();

    if let Err(error) = clients::run_cmd(opts.subcommand) {
        println!("Error: {}", error);
    }
}
