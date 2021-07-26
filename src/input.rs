use crate::billing::Period;
use chrono::{Duration, Local, NaiveDate};
use chrono_utilities::naive::DateTransitions;
use inquire::error::InquireError;
use inquire::{DateSelect, Text};

pub fn new_client() -> Result<(String, String, String), InquireError> {
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

pub fn select_period(
    billed_until: Option<NaiveDate>,
) -> Result<Period, InquireError> {
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

pub fn confirm(question: &str) -> Result<bool, InquireError> {
    let resp = Text::new(&format!("{}", question))
        .with_default("yes")
        .prompt()?
        .to_lowercase();

    Ok(resp == "yes" || resp == "y")
}
