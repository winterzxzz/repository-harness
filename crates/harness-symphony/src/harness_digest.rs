use std::path::Path;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DigestError {
    #[error("could not calculate logical Harness database digest: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub fn logical_digest(path: &Path) -> Result<String, DigestError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut tables_statement = connection.prepare(
        "SELECT name, COALESCE(sql, '') FROM sqlite_master \
         WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let tables = tables_statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut hasher = Sha256::new();
    for (table, schema) in tables {
        hash_bytes(&mut hasher, table.as_bytes());
        hash_bytes(&mut hasher, schema.as_bytes());
        let quoted = table.replace('"', "\"\"");
        let mut statement = connection.prepare(&format!("SELECT * FROM \"{quoted}\""))?;
        let column_count = statement.column_count();
        for name in statement.column_names() {
            hash_bytes(&mut hasher, name.as_bytes());
        }
        let mut rows = statement.query([])?;
        let mut encoded_rows = Vec::new();
        while let Some(row) = rows.next()? {
            let mut encoded = Vec::new();
            for index in 0..column_count {
                encode_value(row.get_ref(index)?, &mut encoded);
            }
            encoded_rows.push(encoded);
        }
        encoded_rows.sort();
        for row in encoded_rows {
            hash_bytes(&mut hasher, &row);
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn encode_value(value: ValueRef<'_>, output: &mut Vec<u8>) {
    match value {
        ValueRef::Null => output.push(0),
        ValueRef::Integer(value) => {
            output.push(1);
            output.extend_from_slice(&value.to_be_bytes());
        }
        ValueRef::Real(value) => {
            output.push(2);
            output.extend_from_slice(&value.to_bits().to_be_bytes());
        }
        ValueRef::Text(value) => {
            output.push(3);
            output.extend_from_slice(&(value.len() as u64).to_be_bytes());
            output.extend_from_slice(value);
        }
        ValueRef::Blob(value) => {
            output.push(4);
            output.extend_from_slice(&(value.len() as u64).to_be_bytes());
            output.extend_from_slice(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use rusqlite::Connection;

    use super::logical_digest;

    fn fixture_db(path: &Path, rows: &[(i64, &str)]) -> PathBuf {
        let connection = Connection::open(path).unwrap();
        connection
            .execute(
                "CREATE TABLE fixture (id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
                [],
            )
            .unwrap();
        for (id, value) in rows {
            connection
                .execute(
                    "INSERT INTO fixture (id, value) VALUES (?1, ?2)",
                    rusqlite::params![id, value],
                )
                .unwrap();
        }
        path.to_path_buf()
    }

    #[test]
    fn logical_digest_ignores_insertion_order_but_detects_content_change() {
        let temp = tempfile::tempdir().unwrap();
        let first = fixture_db(&temp.path().join("first.db"), &[(1, "a"), (2, "b")]);
        let second = fixture_db(&temp.path().join("second.db"), &[(2, "b"), (1, "a")]);
        assert_eq!(
            logical_digest(&first).unwrap(),
            logical_digest(&second).unwrap()
        );

        Connection::open(&second)
            .unwrap()
            .execute("UPDATE fixture SET value='changed' WHERE id=2", [])
            .unwrap();
        assert_ne!(
            logical_digest(&first).unwrap(),
            logical_digest(&second).unwrap()
        );
    }
}
