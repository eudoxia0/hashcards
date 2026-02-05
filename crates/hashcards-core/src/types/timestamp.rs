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

use chrono::NaiveDateTime;
use chrono::SubsecRound;
use serde::Deserialize;
use serde::Serialize;

use crate::error::ErrorReport;
use crate::types::date::Date;

/// A timestamp without a timezone and millisecond precision.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Timestamp(NaiveDateTime);

impl Timestamp {
    pub fn new(ndt: NaiveDateTime) -> Self {
        Self(ndt.trunc_subsecs(3))
    }

    /// Converts a timestamp into a `NaiveDateTime`.
    pub fn into_inner(self) -> NaiveDateTime {
        self.0
    }

    /// The current timestamp in the user's local time.
    #[cfg(feature = "clock")]
    pub fn now() -> Self {
        Self(chrono::Local::now().naive_local().trunc_subsecs(3))
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

impl From<Timestamp> for String {
    fn from(ts: Timestamp) -> String {
        ts.to_string()
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

    #[test]
    fn test_deserialize() {
        let ts: Timestamp = serde_json::from_str("\"2023-10-05T14:30:15.123\"").unwrap();
        let expected_ndt =
            NaiveDateTime::parse_from_str("2023-10-05T14:30:15.123", "%Y-%m-%dT%H:%M:%S%.3f")
                .unwrap();
        assert_eq!(ts.0, expected_ndt);
    }
}
