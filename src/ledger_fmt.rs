use std::fmt;

pub trait LedgerDisplay {
    fn ledger_fmt(&self, buf: &mut dyn fmt::Write) -> fmt::Result;
}

pub fn ledger_fmt(item: impl LedgerDisplay) -> String {
    let mut buf = String::new();
    item.ledger_fmt(&mut buf).expect("String formatting failed");
    buf
}
