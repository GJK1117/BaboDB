use crate::error::{BaboDbError, Result};
use std::io::{ErrorKind, Read, Write};

const OP_PUT: u8 = 1;
const OP_DELETE: u8 = 2;
const HEADER_LEN: usize = 9;
const MAX_KEY_LEN: usize = 1024 * 1024;
const MAX_VALUE_LEN: usize = 64 * 1024 * 1024;

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

    validate_decoded_len(key_len, MAX_KEY_LEN, BaboDbError::KeyTooLarge)?;
    validate_decoded_len(value_len, MAX_VALUE_LEN, BaboDbError::ValueTooLarge)?;

    let Some(key) = read_bytes(reader, key_len)? else {
        return Ok(None);
    };

    match op {
        OP_PUT => {
            let Some(value) = read_bytes(reader, value_len)? else {
                return Ok(None);
            };
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

fn read_bytes(reader: &mut impl Read, len: usize) -> Result<Option<Vec<u8>>> {
    let mut bytes = vec![0_u8; len];

    match reader.read_exact(&mut bytes) {
        Ok(()) => Ok(Some(bytes)),
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => Ok(None),
        Err(error) => Err(error.into()),
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

fn validate_decoded_len(
    len: usize,
    max: usize,
    error: impl FnOnce(usize) -> BaboDbError,
) -> std::result::Result<(), BaboDbError> {
    if len > max {
        return Err(error(len));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{read_record, write_record, LogRecord, MAX_KEY_LEN, OP_PUT};
    use crate::BaboDbError;

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

    #[test]
    fn ignores_truncated_tail_record() {
        let mut bytes = LogRecord::put(b"name", b"babo").unwrap().encode();
        bytes.extend_from_slice(&[OP_PUT, 4, 0, 0]);

        let mut reader = bytes.as_slice();

        assert_eq!(
            read_record(&mut reader).unwrap(),
            Some(LogRecord::put(b"name", b"babo").unwrap())
        );
        assert_eq!(read_record(&mut reader).unwrap(), None);
    }

    #[test]
    fn rejects_oversized_decoded_key_before_allocating() {
        let mut bytes = Vec::new();
        bytes.push(OP_PUT);
        bytes.extend_from_slice(&((MAX_KEY_LEN as u32) + 1).to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());

        let error = read_record(&mut bytes.as_slice()).unwrap_err();

        assert!(matches!(error, BaboDbError::KeyTooLarge(_)));
    }
}
