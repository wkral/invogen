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

mod clients;
mod invoices;
mod error;

use clap::Clap;

#[derive(Clap)]
struct Opts {
    #[clap(subcommand)]
    subcommand: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    Client(clients::Command),
    Invoice(invoices::Command),
}

fn main() {
    let opts = Opts::parse();

    match opts.subcommand {
        SubCommand::Client(cmd) => {
            if let Err(error) = clients::run_cmd(cmd) {
                println!("Error: {}", error);
            }
        }
    }
}
