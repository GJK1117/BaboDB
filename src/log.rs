use crate::error::{BaboDbError, Result};
use std::io::{Read, Write};

const OP_PUT: u8 = 1;
const OP_DELETE: u8 = 2;
const HEADER_LEN: usize = 9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LogRecord {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

impl LogRecord {
    pub(crate) fn put(key: &[u8], value: &[u8]) -> Result<Self> {
        validate_len(key.len(), BaboDbError::KeyTooLarge)?;
        validate_len(value.len(), BaboDbError::ValueTooLarge)?;

        Ok(Self::Put {
            key: key.to_vec(),
            value: value.to_vec(),
        })
    }

    pub(crate) fn delete(key: &[u8]) -> Result<Self> {
        validate_len(key.len(), BaboDbError::KeyTooLarge)?;
        Ok(Self::Delete { key: key.to_vec() })
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(self.encoded_len());

        match self {
            Self::Put { key, value } => {
                encoded.push(OP_PUT);
                encoded.extend_from_slice(&(key.len() as u32).to_le_bytes());
                encoded.extend_from_slice(&(value.len() as u32).to_le_bytes());
                encoded.extend_from_slice(key);
                encoded.extend_from_slice(value);
            }
            Self::Delete { key } => {
                encoded.push(OP_DELETE);
                encoded.extend_from_slice(&(key.len() as u32).to_le_bytes());
                encoded.extend_from_slice(&0_u32.to_le_bytes());
                encoded.extend_from_slice(key);
            }
        }

        encoded
    }

    pub(crate) fn encoded_len(&self) -> usize {
        match self {
            Self::Put { key, value } => HEADER_LEN + key.len() + value.len(),
            Self::Delete { key } => HEADER_LEN + key.len(),
        }
    }
}

pub(crate) fn read_record(reader: &mut impl Read) -> Result<Option<LogRecord>> {
    let mut header = [0_u8; HEADER_LEN];
    let mut read = 0;

    while read < HEADER_LEN {
        let count = reader.read(&mut header[read..])?;
        if count == 0 {
            if read == 0 {
                return Ok(None);
            }

            return Err(BaboDbError::CorruptRecord("truncated header"));
        }
        read += count;
    }

    let op = header[0];
    let key_len = u32::from_le_bytes([header[1], header[2], header[3], header[4]]) as usize;
    let value_len = u32::from_le_bytes([header[5], header[6], header[7], header[8]]) as usize;

    let mut key = vec![0_u8; key_len];
    reader.read_exact(&mut key)?;

    match op {
        OP_PUT => {
            let mut value = vec![0_u8; value_len];
            reader.read_exact(&mut value)?;
            Ok(Some(LogRecord::Put { key, value }))
        }
        OP_DELETE => {
            if value_len != 0 {
                return Err(BaboDbError::CorruptRecord(
                    "delete record has non-zero value length",
                ));
            }
            Ok(Some(LogRecord::Delete { key }))
        }
        _ => Err(BaboDbError::CorruptRecord("unknown operation")),
    }
}

pub(crate) fn write_record(writer: &mut impl Write, record: &LogRecord) -> Result<()> {
    writer.write_all(&record.encode())?;
    writer.flush()?;
    Ok(())
}

fn validate_len(
    len: usize,
    error: impl FnOnce(usize) -> BaboDbError,
) -> std::result::Result<(), BaboDbError> {
    if len > u32::MAX as usize {
        return Err(error(len));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{read_record, write_record, LogRecord};

    #[test]
    fn round_trips_put_record() {
        let record = LogRecord::put(b"name", b"babo").unwrap();
        let mut bytes = Vec::new();

        write_record(&mut bytes, &record).unwrap();

        assert_eq!(read_record(&mut bytes.as_slice()).unwrap(), Some(record));
    }

    #[test]
    fn round_trips_delete_record() {
        let record = LogRecord::delete(b"name").unwrap();
        let mut bytes = Vec::new();

        write_record(&mut bytes, &record).unwrap();

        assert_eq!(read_record(&mut bytes.as_slice()).unwrap(), Some(record));
    }
}
