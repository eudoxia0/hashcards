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

use rusqlite::ToSql;
use rusqlite::types::ToSqlOutput;

use crate::error::ErrorReport;
use crate::error::fail;

#[derive(Debug, PartialEq, Eq)]
pub enum CardType {
    Basic,
    Cloze,
}

impl CardType {
    fn as_str(&self) -> &str {
        match self {
            CardType::Basic => "basic",
            CardType::Cloze => "cloze",
        }
    }
}

impl TryFrom<String> for CardType {
    type Error = ErrorReport;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "basic" => Ok(CardType::Basic),
            "cloze" => Ok(CardType::Cloze),
            _ => fail(format!("Invalid card type: '{}'", value)),
        }
    }
}

impl ToSql for CardType {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::error::Fallible;

    #[test]
    fn test_card_type_serialization_roundtrip() -> Fallible<()> {
        let card_types = [CardType::Basic, CardType::Cloze];
        for ct in card_types {
            assert_eq!(ct, CardType::try_from(ct.as_str().to_string())?);
        }
        Ok(())
    }

    #[test]
    fn test_card_type_invalid() {
        let invalid = "foo".to_string();
        let res = CardType::try_from(invalid);
        assert!(res.is_err());
        let err = res.err().unwrap();
        assert_eq!(err.to_string(), "error: Invalid card type: 'foo'");
    }
}
