// Copyright 2025 Fernando Borretti
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::cmp::Ordering;
use std::fmt::Display;
use std::fmt::Formatter;

use rusqlite::ToSql;
use rusqlite::types::FromSql;
use rusqlite::types::FromSqlError;
use rusqlite::types::FromSqlResult;
use rusqlite::types::ToSqlOutput;
use rusqlite::types::ValueRef;
use serde::Serialize;

use crate::error::ErrorReport;
use crate::error::Fallible;

/// Wrapper around the underlying hash function. Needed because blake3 does
/// not implement Ord and PartialOrd.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CardHash {
    inner: blake3::Hash,
}

impl CardHash {
    #[cfg(test)]
    pub fn hash_bytes(bytes: &[u8]) -> Self {
        Self {
            inner: blake3::hash(bytes),
        }
    }

    pub fn to_hex(self) -> String {
        self.inner.to_hex().to_string()
    }

    pub fn from_hex(s: &str) -> Fallible<Self> {
        let inner = blake3::Hash::from_hex(s)
            .map_err(|_| ErrorReport::new("invalid hash in performance database"))?;
        Ok(Self { inner })
    }
}

impl PartialOrd for CardHash {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CardHash {
    fn cmp(&self, other: &Self) -> Ordering {
        self.inner.as_bytes().cmp(other.inner.as_bytes())
    }
}

impl ToSql for CardHash {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_hex()))
    }
}

impl FromSql for CardHash {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let string: String = FromSql::column_result(value)?;
        CardHash::from_hex(&string).map_err(|e| FromSqlError::Other(Box::new(e)))
    }
}

impl Display for CardHash {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl Serialize for CardHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

pub struct Hasher {
    inner: blake3::Hasher,
}

impl Hasher {
    pub fn new() -> Self {
        Self {
            inner: blake3::Hasher::new(),
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    pub fn finalize(self) -> CardHash {
        CardHash {
            inner: self.inner.finalize(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        let hash = CardHash::hash_bytes(b"test");
        assert_eq!(
            hash.to_string(),
            "4878ca0425c739fa427f7eda20fe845f6b2e46ba5fe2a14df5b1e32f50603215"
        );
    }

    #[test]
    fn test_ordering() -> Fallible<()> {
        let a =
            CardHash::from_hex("0000000000000000000000000000000000000000000000000000000000000000")?;
        let b =
            CardHash::from_hex("0000000000000000000000000000000000000000000000000000000000000001")?;
        let c =
            CardHash::from_hex("0000000000000000000000000000000000000000000000000000000000000002")?;
        assert!(a < b);
        assert!(b < c);
        Ok(())
    }
}
