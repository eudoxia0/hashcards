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

use std::fmt::Display;
use std::fmt::Formatter;

use chrono::Local;
use chrono::NaiveDateTime;
use rusqlite::ToSql;
use rusqlite::types::FromSql;
use rusqlite::types::FromSqlError;
use rusqlite::types::FromSqlResult;
use rusqlite::types::ToSqlOutput;
use rusqlite::types::ValueRef;
use serde::Serialize;

use crate::error::ErrorReport;
use crate::types::date::Date;

/// A timestamp without a timezone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Timestamp(NaiveDateTime);

impl Timestamp {
    /// Converts a timestamp into a `NaiveDateTime`.
    pub fn into_inner(self) -> NaiveDateTime {
        self.0
    }

    /// The current timestamp in the user's local time.
    pub fn now() -> Self {
        Self(Local::now().naive_local())
    }

    /// The date component of this timestamp.
    pub fn date(self) -> Date {
        Date::new(Local::now().naive_local().date())
    }
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.format("%Y-%m-%dT%H:%M:%S").to_string())
    }
}

impl ToSql for Timestamp {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let str: String = self.to_string();
        Ok(ToSqlOutput::from(str))
    }
}

impl FromSql for Timestamp {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let string: String = FromSql::column_result(value)?;
        let ndt: NaiveDateTime = NaiveDateTime::parse_from_str(&string, "%Y-%m-%dT%H:%M:%S")
            .map_err(|_| {
                FromSqlError::Other(Box::new(ErrorReport::new(format!(
                    "Failed to parse timestamp: '{string}'."
                ))))
            })?;
        Ok(Timestamp(ndt))
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = self.to_string();
        serializer.serialize_str(&s)
    }
}
