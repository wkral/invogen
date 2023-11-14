use chrono::{Datelike, Days, Months, NaiveDate};

pub trait DateBoundaries {
    fn start_of_month(&self) -> Option<Self>
    where
        Self: Sized;

    fn end_of_month(&self) -> Option<Self>
    where
        Self: Sized;

    fn start_of_week(&self) -> Option<Self>
    where
        Self: Sized;

    fn end_of_week(&self) -> Option<Self>
    where
        Self: Sized;
}

impl DateBoundaries for NaiveDate {
    fn start_of_month(&self) -> Option<Self> {
        self.with_day(1)
    }

    fn end_of_month(&self) -> Option<Self> {
        self.checked_add_months(Months::new(1))
            .and_then(|d| d.with_day(1))
            .and_then(|d| d.checked_sub_days(Days::new(1)))
    }

    fn start_of_week(&self) -> Option<Self> {
        let days = Days::new(self.weekday().num_days_from_monday().into());
        self.checked_sub_days(days)
    }

    fn end_of_week(&self) -> Option<Self> {
        let max_days = 6;
        let num_days = max_days - self.weekday().num_days_from_monday();
        self.checked_add_days(Days::new(num_days.into()))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn ymd(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn assert_expected_date(
        date: Option<NaiveDate>,
        year: i32,
        month: u32,
        day: u32,
    ) {
        assert_eq!(date, NaiveDate::from_ymd_opt(year, month, day));
    }

    #[test]
    fn end_of_month() {
        assert_expected_date(ymd(2023, 1, 30).end_of_month(), 2023, 1, 31);
        assert_expected_date(ymd(2023, 2, 9).end_of_month(), 2023, 2, 28);
        assert_expected_date(ymd(2024, 2, 9).end_of_month(), 2024, 2, 29);
        assert_expected_date(ymd(2023, 9, 24).end_of_month(), 2023, 9, 30);
        assert_expected_date(ymd(2023, 12, 24).end_of_month(), 2023, 12, 31);
    }

    #[test]
    fn start_of_month() {
        assert_expected_date(ymd(2023, 1, 30).start_of_month(), 2023, 1, 1);
        assert_expected_date(ymd(2023, 2, 9).start_of_month(), 2023, 2, 1);
        assert_expected_date(ymd(2024, 2, 9).start_of_month(), 2024, 2, 1);
    }

    #[test]
    fn end_of_week() {
        assert_expected_date(ymd(2023, 1, 30).end_of_week(), 2023, 2, 5);
        assert_expected_date(ymd(2023, 11, 15).end_of_week(), 2023, 11, 19);
        assert_expected_date(ymd(2023, 11, 12).end_of_week(), 2023, 11, 12);
        assert_expected_date(ymd(2023, 12, 31).end_of_week(), 2023, 12, 31);
        assert_expected_date(ymd(2024, 12, 31).end_of_week(), 2025, 1, 5);
    }

    #[test]
    fn start_of_week() {
        assert_expected_date(ymd(2023, 2, 4).start_of_week(), 2023, 1, 30);
        assert_expected_date(ymd(2023, 11, 15).start_of_week(), 2023, 11, 13);
        assert_expected_date(ymd(2023, 11, 13).start_of_week(), 2023, 11, 13);
        assert_expected_date(ymd(2023, 12, 31).start_of_week(), 2023, 12, 25);
        assert_expected_date(ymd(2025, 1, 4).start_of_week(), 2024, 12, 30);
    }
}
