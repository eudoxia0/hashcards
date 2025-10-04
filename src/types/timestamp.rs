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
use chrono::SubsecRound;
use rusqlite::ToSql;
use rusqlite::types::FromSql;
use rusqlite::types::FromSqlError;
use rusqlite::types::FromSqlResult;
use rusqlite::types::ToSqlOutput;
use rusqlite::types::ValueRef;
use serde::Serialize;

use crate::error::ErrorReport;
use crate::types::date::Date;

/// A timestamp without a timezone and millisecond precision.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Timestamp(NaiveDateTime);

impl Timestamp {
    #[cfg(test)]
    pub fn new(ndt: NaiveDateTime) -> Self {
        Self(ndt.trunc_subsecs(3))
    }

    /// Converts a timestamp into a `NaiveDateTime`.
    pub fn into_inner(self) -> NaiveDateTime {
        self.0
    }

    /// The current timestamp in the user's local time.
    pub fn now() -> Self {
        Self(Local::now().naive_local().trunc_subsecs(3))
    }

    /// The date component of this timestamp.
    pub fn date(self) -> Date {
        Date::new(self.0.date())
    }
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.format("%Y-%m-%dT%H:%M:%S%.3f"))
    }
}

impl TryFrom<String> for Timestamp {
    type Error = ErrorReport;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let ndt = NaiveDateTime::parse_from_str(&value, "%Y-%m-%dT%H:%M:%S%.3f")
            .map_err(|_| ErrorReport::new(format!("Failed to parse timestamp: '{value}'.")))?;
        Ok(Timestamp(ndt))
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
        Timestamp::try_from(string).map_err(|e| FromSqlError::Other(Box::new(e)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_to_string() {
        let ndt = NaiveDateTime::parse_from_str("2023-10-05T14:30:15.123", "%Y-%m-%dT%H:%M:%S%.3f")
            .unwrap();
        let ts = Timestamp(ndt);
        assert_eq!(ts.to_string(), "2023-10-05T14:30:15.123");
    }

    #[test]
    fn test_try_from_string() {
        let s = "2023-10-05T14:30:15.123".to_string();
        let ts = Timestamp::try_from(s).unwrap();
        let expected_ndt =
            NaiveDateTime::parse_from_str("2023-10-05T14:30:15.123", "%Y-%m-%dT%H:%M:%S%.3f")
                .unwrap();
        assert_eq!(ts.0, expected_ndt);
    }

    #[test]
    fn test_serialize() {
        let ndt = NaiveDateTime::parse_from_str("2023-10-05T14:30:15.123", "%Y-%m-%dT%H:%M:%S%.3f")
            .unwrap();
        let ts = Timestamp(ndt);
        let serialized = serde_json::to_string(&ts).unwrap();
        assert_eq!(serialized, "\"2023-10-05T14:30:15.123\"");
    }
}
