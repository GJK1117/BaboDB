# BaboDB

A tiny single-file database in Rust. Small, simple, and built to learn.

BaboDB is currently an early prototype: it is not trying to be SQLite yet. The
first milestone is a minimal embedded key-value store backed by one append-only
file.

## Current features

- Single-file storage
- Append-only log records
- In-memory index rebuilt when the database opens
- Byte-oriented keys and values
- Basic key-value operations:
  - `put`
  - `get`
  - `delete`
  - `scan`

## Example

```rust
use babodb::BaboDb;

fn main() -> babodb::Result<()> {
    let mut db = BaboDb::open("example.babodb")?;

    db.put(b"name", b"babo")?;

    assert_eq!(db.get(b"name"), Some(b"babo".to_vec()));

    db.delete(b"name")?;
    assert_eq!(db.get(b"name"), None);

    Ok(())
}
```

## Design notes

BaboDB writes every change as a log record. On open, it replays the log into an
in-memory `BTreeMap`, so `scan` returns keys in sorted order.

This keeps the prototype simple, but it also means:

- the full log is replayed on startup;
- the full live index is kept in memory;
- compaction is not implemented yet;
- this is a single-process prototype, so do not open the same database file with
  multiple `BaboDb` instances at the same time.

## Development

Format and type-check the project, including unit tests:

```sh
cargo fmt --check
cargo check --tests
```

Run the unit tests:

```sh
cargo test
```

On Windows with the default MSVC Rust toolchain, `cargo test` requires the Visual
Studio C++ build tools so that `link.exe` is available.

## Roadmap

Planned future work:

- log compaction
- page-based storage
- B+Tree-backed indexing
- transactions and crash recovery improvements
- a simple SQL layer
