use std::fmt;

use crate::billing::{Invoice, InvoiceTotal};
use crate::clients::Client;
use crate::run::RunError;

use askama::Template;
use askama::filters::Escaper;

#[derive(Template)]
#[template(path = "invoice.tex")]
struct InvoiceData<'a> {
    invoice: &'a Invoice,
    client_name: &'a str,
    address_lines: Vec<&'a str>,
    total: &'a InvoiceTotal,
}

pub fn invoice<'a>(
    invoice: &'a Invoice,
    client: &'a Client,
) -> Result<(), RunError> {
    let data = InvoiceData {
        invoice,
        client_name: client.name.as_str(),
        address_lines: client.address.split('\n').collect(),
        total: &invoice.calculate(),
    };

    println!("{}", data.render()?);

    Ok(())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Tex;

impl Escaper for Tex {
    fn write_escaped_str<W>(&self, mut fmt: W, string: &str) -> fmt::Result
    where
        W: fmt::Write,
    {
        for c in string.chars() {
            match c {
                '%' => fmt.write_str("\\%")?,
                '$' => fmt.write_str("\\$")?,
                _ => fmt.write_char(c)?,
            }
        }
        Ok(())
    }
}
