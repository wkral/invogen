use crate::billing::{Currency, Money, Period, Rate, TaxRate, Unit};
use crate::calendar::DateBoundaries;

use chrono::{Duration, Local, NaiveDate};
use inquire::{
    error::InquireError, formatter::CustomTypeFormatter, Confirm, CustomType,
    DateSelect, Select, Text,
};
use rust_decimal::Decimal;
use strum::VariantNames;

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
        let should_break = line.is_empty();
        addr_lines.push(line);

        if should_break {
            break;
        }
    }
    Ok(addr_lines.join("\n").trim().to_string())
}

pub fn period(billed_until: Option<NaiveDate>) -> InputResult<Period> {
    let today = Local::now().date_naive();
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

pub fn num_hours() -> InputResult<Decimal> {
    let formatter: CustomTypeFormatter<Decimal> = &|i| format!("{:.0}", i);
    let amount: Decimal = CustomType::new("Billable Hours:")
        .with_formatter(formatter)
        .with_error_message("Please type a valid number")
        .prompt()?;
    Ok(amount)
}

pub fn paid_date(issue_date: NaiveDate) -> InputResult<NaiveDate> {
    let today = Local::now().date_naive();

    DateSelect::new("Paid on:")
        .with_min_date(issue_date)
        .with_max_date(today)
        .prompt()
}

pub fn service_select(services: Vec<&str>) -> InputResult<String> {
    let service = Select::new("Service:", services)
        .with_vim_mode(true)
        .prompt()?;

    Ok(service.to_string())
}

pub fn service() -> InputResult<(String, Rate, NaiveDate)> {
    let name = Text::new("Service:").prompt()?;
    let (rate, effective) = rate()?;

    Ok((name, rate, effective))
}

pub fn rate() -> InputResult<(Rate, NaiveDate)> {
    let formatter: CustomTypeFormatter<Decimal> = &|i| format!("${:.2}", i);
    let amount: Decimal = CustomType::new("Amount:")
        .with_formatter(formatter)
        .with_error_message("Please type a valid number")
        .prompt()?;
    let currency = Select::new("Currency:", Currency::VARIANTS.to_vec())
        .with_vim_mode(true)
        .prompt()?;

    let unit = Select::new("Per:", Unit::VARIANTS.to_vec())
        .with_vim_mode(true)
        .prompt()?;

    let effective = DateSelect::new("Effective:").prompt()?;
    let rate = Rate {
        amount: Money::new(
            Currency::from_str(currency).expect("only selecting from variants"),
            amount,
        ),
        per: Unit::from_str(unit).expect("only selecting from variants"),
    };
    Ok((rate, effective))
}

pub fn taxes() -> InputResult<(Vec<TaxRate>, NaiveDate)> {
    let mut taxes: Vec<TaxRate> = Vec::new();

    let formatter: CustomTypeFormatter<i64> = &|i| format!("{}%", i);
    loop {
        let name = Text::new("Tax name:").prompt()?;
        let percentage: i64 = CustomType::new("Percentage:")
            .with_formatter(formatter)
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

pub fn another() -> InputResult<bool> {
    Confirm::new("Add another").with_default(false).prompt()
}
