use std::fmt;

#[derive(Debug)]
pub enum ClobError {
    InstrumentNotFound(String),
    ParticipantNotFound(String),
    OrderNotFound(String),
    InstrumentHalted(String),
    InvalidPrice(String),
    InvalidQty(String),
    RiskLimitExceeded(String),
    IoError(String),
}

impl fmt::Display for ClobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClobError::InstrumentNotFound(s) => write!(f, "instrument not found: {}", s),
            ClobError::ParticipantNotFound(s) => write!(f, "participant not found: {}", s),
            ClobError::OrderNotFound(s) => write!(f, "order not found: {}", s),
            ClobError::InstrumentHalted(s) => write!(f, "instrument halted: {}", s),
            ClobError::InvalidPrice(s) => write!(f, "invalid price: {}", s),
            ClobError::InvalidQty(s) => write!(f, "invalid quantity: {}", s),
            ClobError::RiskLimitExceeded(s) => write!(f, "risk limit exceeded: {}", s),
            ClobError::IoError(s) => write!(f, "I/O error: {}", s),
        }
    }
}

impl std::error::Error for ClobError {}
