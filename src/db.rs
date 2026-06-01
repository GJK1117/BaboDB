use crate::error::BaboDbError;
use crate::error::Result;
use crate::log::{read_record, write_record, LogRecord};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Seek, SeekFrom};
use std::path::Path;

const MAX_LOG_LEN: u64 = 1024 * 1024 * 1024;

/// A tiny single-file key-value database.
///
/// `BaboDb` is a single-process prototype. Do not open the same database file
/// through multiple `BaboDb` instances at the same time.
pub struct BaboDb {
    file: File,
    index: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl BaboDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let read_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let index = load_index(&read_file)?;
        let mut file = read_file;
        file.seek(SeekFrom::End(0))?;

        Ok(Self { file, index })
    }

    pub fn put(&mut self, key: impl AsRef<[u8]>, value: impl AsRef<[u8]>) -> Result<()> {
        let key = key.as_ref();
        let value = value.as_ref();
        let record = LogRecord::put(key, value)?;

        write_record(&mut self.file, &record)?;
        self.file.sync_data()?;
        self.index.insert(key.to_vec(), value.to_vec());

        Ok(())
    }

    pub fn get(&self, key: impl AsRef<[u8]>) -> Option<Vec<u8>> {
        self.index.get(key.as_ref()).cloned()
    }

    pub fn delete(&mut self, key: impl AsRef<[u8]>) -> Result<bool> {
        let key = key.as_ref();
        let existed = self.index.contains_key(key);
        let record = LogRecord::delete(key)?;

        write_record(&mut self.file, &record)?;
        self.file.sync_data()?;
        self.index.remove(key);

        Ok(existed)
    }

    pub fn scan(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.index
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
}

fn load_index(file: &File) -> Result<BTreeMap<Vec<u8>, Vec<u8>>> {
    let log_len = file.metadata()?.len();
    if log_len > MAX_LOG_LEN {
        return Err(BaboDbError::LogTooLarge(log_len));
    }

    let mut reader = BufReader::new(file.try_clone()?);
    let mut index = BTreeMap::new();

    while let Some(record) = read_record(&mut reader)? {
        match record {
            LogRecord::Put { key, value } => {
                index.insert(key, value);
            }
            LogRecord::Delete { key } => {
                index.remove(&key);
            }
        }
    }

    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::BaboDb;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn puts_gets_deletes_and_scans_values() {
        let path = test_path("basic");

        {
            let mut db = BaboDb::open(&path).unwrap();

            db.put(b"name", b"babo").unwrap();
            db.put(b"lang", b"rust").unwrap();

            assert_eq!(db.get(b"name"), Some(b"babo".to_vec()));
            assert_eq!(
                db.scan(),
                vec![
                    (b"lang".to_vec(), b"rust".to_vec()),
                    (b"name".to_vec(), b"babo".to_vec())
                ]
            );
            assert!(db.delete(b"name").unwrap());
            assert_eq!(db.get(b"name"), None);
            assert!(!db.delete(b"missing").unwrap());
        }

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rebuilds_index_when_reopened() {
        let path = test_path("reopen");

        {
            let mut db = BaboDb::open(&path).unwrap();
            db.put(b"name", b"first").unwrap();
            db.put(b"name", b"second").unwrap();
            db.put(b"temp", b"value").unwrap();
            db.delete(b"temp").unwrap();
        }

        let db = BaboDb::open(&path).unwrap();

        assert_eq!(db.get(b"name"), Some(b"second".to_vec()));
        assert_eq!(db.get(b"temp"), None);

        fs::remove_file(path).unwrap();
    }

    fn test_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("babodb-{name}-{nanos}.db"))
    }
}
