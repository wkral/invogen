use chrono::NaiveDate;
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("IO Error: {source}")]
    Io {
        #[from]
        source: io::Error,
    },

    #[error("Error decoding history: {source}")]
    Format {
        #[from]
        source: serde_lexpr::Error,
    },

    #[error("No client found for: '{key}'")]
    NotFound { key: String },

    #[error("No effective rate found for: '{key}' as of {effective}")]
    NoRate { key: String, effective: NaiveDate },

    #[error("Invoices are out of sequence: {found} found after {current}")]
    InvoiceOutOfSequence { current: usize, found: usize },

    #[error("{client} invoice #{number} recoded as paid before it exists")]
    PaidOutOfSequence { client: String, number: usize },

    #[error("{client} invoice {number} recoded as paid twice")]
    AlreadyPaid { client: String, number: usize },

    #[error("Input Error: {source}")]
    Input {
        #[from]
        source: inquire::error::InquireError,
    },

}
