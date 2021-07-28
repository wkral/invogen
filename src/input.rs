use crate::billing::{Period, TaxRate};
use chrono::{Duration, Local, NaiveDate};
use chrono_utilities::naive::DateTransitions;
use inquire::{error::InquireError, Confirm, CustomType, DateSelect, Text};

type InputResult<T> = Result<T, InquireError>;

pub fn new_client() -> InputResult<(String, String, String)> {
    let key = Text::new("Client key:").prompt()?.to_lowercase();
    let name = Text::new("Name:").prompt()?;
    let mut count = 0;
    let mut addr_lines: Vec<String> = Vec::new();
    loop {
        count += 1;

        let line = Text::new(&format!("Address Line {}:", count))
            .with_help_message("Hit <enter> on an empty line to stop input")
            .prompt()?;
        let should_break = line == "";
        addr_lines.push(line);

        if should_break {
            break;
        }
    }

    Ok((key, name, addr_lines.join("\n").trim().to_string()))
}

pub fn select_period(billed_until: Option<NaiveDate>) -> InputResult<Period> {
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

pub fn select_taxes() -> InputResult<(Vec<TaxRate>, NaiveDate)> {
    let mut taxes: Vec<TaxRate> = Vec::new();

    loop {
        let name = Text::new("Tax name:").prompt()?;
        let percentage: u8 = CustomType::new("Percentage:")
            .with_formatter(&|i| format!("{}%", i))
            .with_error_message("Please type a valid number")
            .prompt()?;

        taxes.push(TaxRate { name, percentage });

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
