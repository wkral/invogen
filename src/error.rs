use std::io;
use chrono::NaiveDate;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("IO Error")]
    Io {
        #[from]
        source: io::Error,
    },

    #[error("Format Error")]
    Format {
        #[from]
        source: serde_lexpr::Error,
    },

    #[error("No client found for: \'{key}\'")]
    NotFound {
        key: String
    },

    #[error("No effective rate found for: \'{key}\'")]
    NoRate {
        key: String,
        effective: NaiveDate,
    },

}
