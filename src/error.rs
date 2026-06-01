use std::fmt::{Display, Formatter};

pub type Result<T> = std::result::Result<T, BaboDbError>;

#[derive(Debug)]
pub enum BaboDbError {
    Io(std::io::Error),
    CorruptRecord(&'static str),
    KeyTooLarge(usize),
    ValueTooLarge(usize),
}

impl Display for BaboDbError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::CorruptRecord(message) => write!(f, "corrupt record: {message}"),
            Self::KeyTooLarge(size) => write!(f, "key is too large: {size} bytes"),
            Self::ValueTooLarge(size) => write!(f, "value is too large: {size} bytes"),
        }
    }
}

impl std::error::Error for BaboDbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::CorruptRecord(_) | Self::KeyTooLarge(_) | Self::ValueTooLarge(_) => None,
        }
    }
}

impl From<std::io::Error> for BaboDbError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
