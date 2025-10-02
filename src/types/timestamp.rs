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

use chrono::DateTime;
use chrono::Datelike;
use chrono::Duration;
use chrono::Local;
use chrono::TimeZone;
use chrono::Utc;
use rusqlite::ToSql;
use rusqlite::types::FromSql;
use rusqlite::types::FromSqlError;
use rusqlite::types::FromSqlResult;
use rusqlite::types::ToSqlOutput;
use rusqlite::types::ValueRef;

use crate::types::date::Date;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    #[cfg(test)]
    pub fn new(ts: DateTime<Utc>) -> Self {
        Self(ts)
    }

    pub fn into_inner(self) -> DateTime<Utc> {
        self.0
    }

    pub fn now() -> Self {
        Self(Utc::now())
    }

    pub fn local_date(self) -> Date {
        let ts = self.0.with_timezone(&Local);
        Date::new(ts.date_naive())
    }

    /// Returns the range of timestamps that comprise the (local) day around
    /// the given timestamp.
    pub fn day_range(self) -> (Self, Self) {
        let Self(ts) = self;

        // Start of day.
        let start_local: DateTime<Local> = Local
            .with_ymd_and_hms(ts.year(), ts.month(), ts.day(), 0, 0, 0)
            .unwrap();

        // End of day.
        let end_local: DateTime<Local> = start_local + Duration::days(1);

        // Convert to UTC.
        let start_utc: DateTime<Utc> = start_local.with_timezone(&Utc);
        let end_utc: DateTime<Utc> = end_local.with_timezone(&Utc);

        (Self(start_utc), Self(end_utc))
    }
}

impl ToSql for Timestamp {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let str = self.0.to_rfc3339();
        Ok(ToSqlOutput::from(str))
    }
}

impl FromSql for Timestamp {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let string: String = FromSql::column_result(value)?;
        let ts =
            DateTime::parse_from_rfc3339(&string).map_err(|e| FromSqlError::Other(Box::new(e)))?;
        let ts = ts.with_timezone(&Utc);
        Ok(Timestamp(ts))
    }
}
