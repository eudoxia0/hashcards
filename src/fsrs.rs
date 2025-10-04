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
use rusqlite::types::FromSql;
use rusqlite::types::FromSqlError;
use rusqlite::types::FromSqlResult;
use rusqlite::types::ToSqlOutput;
use rusqlite::types::ValueRef;
use serde::Serialize;

use crate::error::ErrorReport;
use crate::error::fail;

pub const W: [f64; 19] = [
    0.40255, 1.18385, 3.173, 15.69105, 7.1949, 0.5345, 1.4604, 0.0046, 1.54575, 0.1192, 1.01925,
    1.9395, 0.11, 0.29605, 2.2698, 0.2315, 2.9898, 0.51655, 0.6621,
];

pub type Recall = f64;
pub type Stability = f64;
pub type Difficulty = f64;

#[derive(Clone, Copy, PartialEq, Debug, Serialize)]
pub enum Grade {
    Forgot,
    Hard,
    Good,
    Easy,
}

impl From<Grade> for f64 {
    fn from(g: Grade) -> f64 {
        match g {
            Grade::Forgot => 1.0,
            Grade::Hard => 2.0,
            Grade::Good => 3.0,
            Grade::Easy => 4.0,
        }
    }
}

impl Grade {
    pub fn as_str(&self) -> &str {
        match self {
            Grade::Forgot => "forgot",
            Grade::Hard => "hard",
            Grade::Good => "good",
            Grade::Easy => "easy",
        }
    }
}

impl TryFrom<String> for Grade {
    type Error = ErrorReport;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "forgot" => Ok(Grade::Forgot),
            "hard" => Ok(Grade::Hard),
            "good" => Ok(Grade::Good),
            "easy" => Ok(Grade::Easy),
            _ => fail("invalid grade string: {value}"),
        }
    }
}

impl ToSql for Grade {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for Grade {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let string: String = FromSql::column_result(value)?;
        Grade::try_from(string).map_err(|e| FromSqlError::Other(Box::new(e)))
    }
}

pub type Interval = f64;

const F: f64 = 19.0 / 81.0;
const C: f64 = -0.5;

pub fn retrievability(t: Interval, s: Stability) -> Recall {
    (1.0 + F * (t / s)).powf(C)
}

pub fn interval(r_d: Recall, s: Stability) -> Interval {
    (s / F) * (r_d.powf(1.0 / C) - 1.0)
}

pub fn initial_stability(g: Grade) -> Stability {
    match g {
        Grade::Forgot => W[0],
        Grade::Hard => W[1],
        Grade::Good => W[2],
        Grade::Easy => W[3],
    }
}

fn s_success(d: Difficulty, s: Stability, r: Recall, g: Grade) -> Stability {
    let t_d = 11.0 - d;
    let t_s = s.powf(-W[9]);
    let t_r = f64::exp(W[10] * (1.0 - r)) - 1.0;
    let h = if g == Grade::Hard { W[15] } else { 1.0 };
    let b = if g == Grade::Easy { W[16] } else { 1.0 };
    let c = f64::exp(W[8]);
    let alpha = 1.0 + t_d * t_s * t_r * h * b * c;
    s * alpha
}

fn s_fail(d: Difficulty, s: Stability, r: Recall) -> Stability {
    let d_f = d.powf(-W[12]);
    let s_f = (s + 1.0).powf(W[13]) - 1.0;
    let r_f = f64::exp(W[14] * (1.0 - r));
    let c_f = W[11];
    let s_f = d_f * s_f * r_f * c_f;
    f64::min(s_f, s)
}

pub fn new_stability(d: Difficulty, s: Stability, r: Recall, g: Grade) -> Stability {
    if g == Grade::Forgot {
        s_fail(d, s, r)
    } else {
        s_success(d, s, r, g)
    }
}

fn clamp_d(d: Difficulty) -> Difficulty {
    d.clamp(1.0, 10.0)
}

pub fn initial_difficulty(g: Grade) -> Difficulty {
    let g: f64 = g.into();
    clamp_d(W[4] - f64::exp(W[5] * (g - 1.0)) + 1.0)
}

pub fn new_difficulty(d: Difficulty, g: Grade) -> Difficulty {
    clamp_d(W[7] * initial_difficulty(Grade::Easy) + (1.0 - W[7]) * dp(d, g))
}

fn dp(d: Difficulty, g: Grade) -> f64 {
    d + delta_d(g) * ((10.0 - d) / 9.0)
}

fn delta_d(g: Grade) -> f64 {
    let g: f64 = g.into();
    -W[6] * (g - 3.0)
}

#[cfg(test)]
mod tests {
    use std::iter::zip;

    use super::*;
    use crate::error::Fallible;

    /// Approximate equality.
    fn feq(a: f64, b: f64) -> bool {
        f64::abs(a - b) < 0.01
    }

    /// R_d = 0.9, I(S) = S.
    #[test]
    fn test_interval_equals_stability() {
        let samples = 100;
        let start = 0.1;
        let end = 5.0;
        let step = (end - start) / (samples as f64 - 1.0);
        for i in 0..samples {
            let s = start + (i as f64) * step;
            assert!(feq(interval(0.9, s), s))
        }
    }

    /// D_0(1) = w_4
    #[test]
    fn test_initial_difficulty_of_forgetting() {
        assert_eq!(initial_difficulty(Grade::Forgot), W[4])
    }

    /// A simulation step.
    #[derive(Clone, Copy, Debug)]
    struct Step {
        /// The time when the review took place.
        t: Interval,
        /// New stability.
        s: Stability,
        /// New difficulty.
        d: Difficulty,
        /// Next interval.
        i: Interval,
    }

    impl PartialEq for Step {
        fn eq(&self, other: &Self) -> bool {
            feq(self.t, other.t)
                && feq(self.s, other.s)
                && feq(self.d, other.d)
                && feq(self.i, other.i)
        }
    }

    /// Simulate a series of reviews.
    fn sim(grades: Vec<Grade>) -> Vec<Step> {
        let mut t: Interval = 0.0;
        let r_d: f64 = 0.9;
        let mut steps = vec![];

        // Initial review.
        assert!(!grades.is_empty());
        let mut grades = grades.clone();
        let g: Grade = grades.remove(0);
        let mut s: Stability = initial_stability(g);
        let mut d: Difficulty = initial_difficulty(g);
        let mut i: Interval = f64::max(interval(r_d, s).round(), 1.0);
        steps.push(Step { t, s, d, i });

        // n-th review
        for g in grades {
            t += i;
            let r: Recall = retrievability(i, s);
            s = new_stability(d, s, r, g);
            d = new_difficulty(d, g);
            i = f64::max(interval(r_d, s).round(), 1.0);
            steps.push(Step { t, s, d, i });
        }

        steps
    }

    /// Test a sequence of three easies.
    #[test]
    fn test_3e() {
        let g = Grade::Easy;
        let grades = vec![g, g, g];
        let expected = vec![
            Step {
                t: 0.0,
                s: 15.69,
                d: 3.22,
                i: 16.0,
            },
            Step {
                t: 16.0,
                s: 150.28,
                d: 2.13,
                i: 150.0,
            },
            Step {
                t: 166.0,
                s: 1252.22,
                d: 1.0,
                i: 1252.0,
            },
        ];
        let actual = sim(grades);
        assert_eq!(expected.len(), actual.len());
        for (expected, actual) in zip(expected, actual) {
            assert_eq!(actual, expected);
        }
    }

    /// Test a sequence of three goods.
    #[test]
    fn test_3g() {
        let g = Grade::Good;
        let grades = vec![g, g, g];
        let expected = vec![
            Step {
                t: 0.0,
                s: 3.17,
                d: 5.28,
                i: 3.0,
            },
            Step {
                t: 3.0,
                s: 10.73,
                d: 5.27,
                i: 11.0,
            },
            Step {
                t: 14.0,
                s: 34.57,
                d: 5.26,
                i: 35.0,
            },
        ];
        let actual = sim(grades);
        assert_eq!(expected.len(), actual.len());
        for (expected, actual) in zip(expected, actual) {
            assert_eq!(actual, expected);
        }
    }

    /// Test a sequence of two hards.
    #[test]
    fn test_2h() {
        let g = Grade::Hard;
        let grades = vec![g, g];
        let expected = vec![
            Step {
                t: 0.0,
                s: 1.18,
                d: 6.48,
                i: 1.0,
            },
            Step {
                t: 1.0,
                s: 1.70,
                d: 7.04,
                i: 2.0,
            },
        ];
        let actual = sim(grades);
        assert_eq!(expected.len(), actual.len());
        for (expected, actual) in zip(expected, actual) {
            assert_eq!(actual, expected);
        }
    }

    /// Test a sequence of two forgots.
    #[test]
    fn test_2f() {
        let g = Grade::Forgot;
        let grades = vec![g, g];
        let expected = vec![
            Step {
                t: 0.0,
                s: 0.40,
                d: 7.19,
                i: 1.0,
            },
            Step {
                t: 1.0,
                s: 0.26,
                d: 8.08,
                i: 1.0,
            },
        ];
        let actual = sim(grades);
        assert_eq!(expected.len(), actual.len());
        for (expected, actual) in zip(expected, actual) {
            assert_eq!(actual, expected);
        }
    }

    /// Test a sequence of good then forgot.
    #[test]
    fn test_gf() {
        let grades = vec![Grade::Good, Grade::Forgot];
        let expected = vec![
            Step {
                t: 0.0,
                s: 3.17,
                d: 5.28,
                i: 3.0,
            },
            Step {
                t: 3.0,
                s: 1.06,
                d: 6.8,
                i: 1.0,
            },
        ];
        let actual = sim(grades);
        assert_eq!(expected.len(), actual.len());
        for (expected, actual) in zip(expected, actual) {
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_grade_serialization_roundtrip() -> Fallible<()> {
        let grades = [Grade::Forgot, Grade::Hard, Grade::Good, Grade::Easy];
        for grade in grades {
            assert_eq!(grade, Grade::try_from(grade.as_str().to_string())?);
        }
        Ok(())
    }

    /// Test the serialization format of Grade.
    #[test]
    fn test_grade_serialization_format() -> Fallible<()> {
        let grades = [Grade::Forgot, Grade::Hard, Grade::Good, Grade::Easy];
        let expected = ["Forgot", "Hard", "Good", "Easy"];
        for (grade, expected) in zip(grades, expected) {
            let serialized = serde_json::to_string(&grade)?;
            let expected = format!("\"{}\"", expected);
            assert_eq!(serialized, expected);
        }
        Ok(())
    }
}
