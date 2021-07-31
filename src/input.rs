use crate::billing::{Currency, Money, Period, Rate, TaxRate, Unit};
use chrono::{Duration, Local, NaiveDate};
use chrono_utilities::naive::DateTransitions;
use inquire::{
    error::InquireError, Confirm, CustomType, DateSelect, Select, Text,
};
use rust_decimal::Decimal;
use strum::VariantNames;

use std::collections::BTreeSet;
use std::str::FromStr;

type InputResult<T> = Result<T, InquireError>;

pub fn client() -> InputResult<(String, String, String)> {
    let key = Text::new("Client key:")
        .with_help_message("This value cannot be changed once set")
        .prompt()?
        .to_lowercase();
    let name = name()?;
    let address = address()?;

    Ok((key, name, address))
}

pub fn name() -> InputResult<String> {
    Text::new("Name:").prompt()
}

pub fn address() -> InputResult<String> {
    let mut count = 0;
    let mut addr_lines: Vec<String> = Vec::new();
    loop {
        count += 1;

        let line = Text::new(&format!("Address line {}:", count))
            .with_help_message("Hit <enter> on an empty line to stop input")
            .prompt()?;
        let should_break = line == "";
        addr_lines.push(line);

        if should_break {
            break;
        }
    }
    Ok(addr_lines.join("\n").trim().to_string())
}

pub fn period(billed_until: Option<NaiveDate>) -> InputResult<Period> {
    let today = Local::today().naive_local();
    let cur_eom = today
        .end_of_month()
        .expect("Error in chrono-utilities end_of_month");

    let from_select = DateSelect::new("Invoice from:").with_max_date(cur_eom);

    let from = match billed_until {
        None => from_select,
        Some(date) => from_select.with_min_date(date),
    }
    .prompt()?;

    let after_from = from + Duration::days(1);
    let from_eom = from
        .end_of_month()
        .expect("Error in chrono-utilities end_of_month");

    let until = DateSelect::new("until:")
        .with_default(from_eom)
        .with_min_date(after_from)
        .with_max_date(cur_eom)
        .prompt()?;

    Ok(Period::new(from, until))
}

pub fn service_description<'a>(
    past_services: &BTreeSet<String>,
) -> InputResult<String> {
    Text::new("Provided service:")
        .with_suggester(&|val: &str| {
            past_services
                .iter()
                .filter(|service| {
                    service.to_lowercase().contains(&val.to_lowercase())
                })
                .map(|s| s.to_string())
                .collect()
        })
        .prompt()
}

pub fn rate() -> InputResult<(Rate, NaiveDate)> {
    let amount: Decimal = CustomType::new("Amount:")
        .with_formatter(&|i| format!("${:.2}", i))
        .with_error_message("Please type a valid number")
        .prompt()?;
    let currency = Select::new("Currency:", &Currency::VARIANTS)
        .with_vim_mode(true)
        .prompt()?
        .value;

    let unit = Select::new("Per:", &Unit::VARIANTS)
        .with_vim_mode(true)
        .prompt()?
        .value;

    let effective = DateSelect::new("Effective:").prompt()?;
    let rate = Rate {
        amount: Money::new(
            Currency::from_str(&currency)
                .expect("only selecting from variants"),
            amount,
        ),
        per: Unit::from_str(&unit).expect("only selecting from variants"),
    };
    Ok((rate, effective))
}

pub fn taxes() -> InputResult<(Vec<TaxRate>, NaiveDate)> {
    let mut taxes: Vec<TaxRate> = Vec::new();

    loop {
        let name = Text::new("Tax name:").prompt()?;
        let percentage: i64 = CustomType::new("Percentage:")
            .with_formatter(&|i| format!("{}%", i))
            .with_error_message("Please type a valid number")
            .prompt()?;

        taxes.push(TaxRate::new(name, percentage));

        if !Confirm::new("Add another").with_default(false).prompt()? {
            break;
        }
    }

    let effective = DateSelect::new("Effective:").prompt()?;
    Ok((taxes, effective))
}

pub fn confirm() -> InputResult<bool> {
    Confirm::new("Confirm").with_default(true).prompt()
}
