// Copyright 2025–2026 Fernando Borretti
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

use std::collections::HashMap;

use chrono::Datelike;
use chrono::Duration;
use maud::Markup;
use maud::html;

use crate::db::Database;
use crate::error::Fallible;
use crate::types::date::Date;

const WEEKS: i64 = 12;
const DAYS_PER_WEEK: i64 = 7;

/// A cell in the heatmap grid as it will be rendered.
struct RenderedCell {
    class: &'static str,
    background: String,
    title: String,
}

/// A single cell of raw data before rendering.
#[derive(Debug, PartialEq)]
struct Cell {
    /// The date this cell represents, or `None` if it's a future day in the
    /// current week (a gap in the last column).
    date: Option<Date>,
    /// The number of cards reviewed on that date. Zero for gaps.
    count: usize,
}

/// Build the 12×7 grid. The outer vec is indexed by column (week), with the
/// oldest week first and the current week last. The inner vec is indexed by
/// weekday, with Sunday at index 0 and Saturday at index 6.
fn build_grid(today: Date, counts: &HashMap<Date, usize>) -> Vec<Vec<Cell>> {
    let today_naive = today.into_inner();
    let sun_offset = today_naive.weekday().num_days_from_sunday() as i64;
    let start_sunday = today_naive - Duration::days(sun_offset + 7 * (WEEKS - 1));
    let mut grid: Vec<Vec<Cell>> = Vec::with_capacity(WEEKS as usize);
    for week in 0..WEEKS {
        let mut col: Vec<Cell> = Vec::with_capacity(DAYS_PER_WEEK as usize);
        for day in 0..DAYS_PER_WEEK {
            let cell_date = start_sunday + Duration::days(7 * week + day);
            if cell_date > today_naive {
                col.push(Cell {
                    date: None,
                    count: 0,
                });
            } else {
                let date = Date::new(cell_date);
                let count = counts.get(&date).copied().unwrap_or(0);
                col.push(Cell {
                    date: Some(date),
                    count,
                });
            }
        }
        grid.push(col);
    }
    grid
}

/// CSS color for a cell. Zero counts get a "neutral" color via CSS variable;
/// non-zero counts get a green whose lightness scales linearly between `min`
/// and `max`.
fn color_for(count: usize, min: usize, max: usize) -> String {
    if count == 0 {
        return "var(--heatmap-empty)".to_string();
    }
    let t = if max == min {
        1.0
    } else {
        (count - min) as f64 / (max - min) as f64
    };
    let lightness = 82.0 - (82.0 - 28.0) * t;
    format!("hsl(135, 55%, {:.1}%)", lightness)
}

fn render_cell(cell: &Cell, min: usize, max: usize) -> RenderedCell {
    match cell.date {
        Some(date) => RenderedCell {
            class: "cell",
            background: color_for(cell.count, min, max),
            title: format!("{}: {} cards reviewed", date, cell.count),
        },
        None => RenderedCell {
            class: "cell empty",
            background: String::new(),
            title: String::new(),
        },
    }
}

/// Render a 12-week × 7-day calendar heatmap of cards reviewed per day.
pub fn render_heatmap(db: &Database, today: Date) -> Fallible<Markup> {
    let today_naive = today.into_inner();
    let sun_offset = today_naive.weekday().num_days_from_sunday() as i64;
    let start_sunday = today_naive - Duration::days(sun_offset + 7 * (WEEKS - 1));
    let counts = db.review_counts_in_range(Date::new(start_sunday), today)?;
    let grid = build_grid(today, &counts);

    let nonzero: Vec<usize> = grid
        .iter()
        .flatten()
        .filter_map(|c| if c.count > 0 { Some(c.count) } else { None })
        .collect();

    if nonzero.is_empty() {
        return Ok(html! {
            div.heatmap.empty {
                "No data."
            }
        });
    }

    let min = nonzero.iter().min().copied().unwrap_or(0);
    let max = nonzero.iter().max().copied().unwrap_or(0);

    // `rows[r][c]` is the cell at row r, column c. Pre-rendering keeps the
    // template simple.
    let mut rows: Vec<Vec<RenderedCell>> = (0..DAYS_PER_WEEK as usize)
        .map(|_| Vec::with_capacity(WEEKS as usize))
        .collect();
    for col in &grid {
        for (r, cell) in col.iter().enumerate() {
            rows[r].push(render_cell(cell, min, max));
        }
    }

    Ok(html! {
        div.heatmap {
            table {
                tbody {
                    @for row in &rows {
                        tr {
                            @for cell in row {
                                td class=(cell.class)
                                   style=(format!("background-color: {};", cell.background))
                                   title=(cell.title) {}
                            }
                        }
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::NaiveDate;

    use super::*;

    fn date(s: &str) -> Date {
        Date::try_from(s.to_string()).unwrap()
    }

    #[test]
    fn test_color_for_zero_is_neutral() {
        assert_eq!(color_for(0, 1, 10), "var(--heatmap-empty)");
    }

    #[test]
    fn test_color_for_min_and_max_differ() {
        let lo = color_for(1, 1, 10);
        let hi = color_for(10, 1, 10);
        assert!(lo.starts_with("hsl("));
        assert!(hi.starts_with("hsl("));
        assert_ne!(lo, hi);
    }

    #[test]
    fn test_color_for_min_equals_max() {
        // Single distinct non-zero value: use the darkest shade.
        let c = color_for(5, 5, 5);
        assert_eq!(c, color_for(10, 1, 10));
    }

    #[test]
    fn test_build_grid_shape_and_anchors() {
        // Sunday 2026-05-24.
        let today = Date::new(NaiveDate::from_ymd_opt(2026, 5, 24).unwrap());
        let grid: Vec<Vec<Cell>> = build_grid(today, &HashMap::new());
        assert_eq!(grid.len(), 12);
        for col in &grid {
            assert_eq!(col.len(), 7);
        }
        // Last column, row 0 (Sunday) is today.
        assert_eq!(grid[11][0].date, Some(today));
        // Last column, rows 1..6 are future days.
        for cell in grid[11].iter().take(7).skip(1) {
            assert_eq!(cell.date, None);
        }
        // First column, row 0 (Sunday) is 11 weeks before today.
        let expected_first =
            Date::new(NaiveDate::from_ymd_opt(2026, 5, 24).unwrap() - Duration::days(7 * 11));
        assert_eq!(grid[0][0].date, Some(expected_first));
    }

    #[test]
    fn test_build_grid_midweek() {
        // Wednesday 2026-05-20: Sunday of this week is 2026-05-17.
        let today = Date::new(NaiveDate::from_ymd_opt(2026, 5, 20).unwrap());
        let grid = build_grid(today, &HashMap::new());
        assert_eq!(grid[11][0].date, Some(date("2026-05-17"))); // Sun
        assert_eq!(grid[11][1].date, Some(date("2026-05-18"))); // Mon
        assert_eq!(grid[11][2].date, Some(date("2026-05-19"))); // Tue
        assert_eq!(grid[11][3].date, Some(today)); // Wed
        // Thu/Fri/Sat are future.
        assert_eq!(grid[11][4].date, None);
        assert_eq!(grid[11][5].date, None);
        assert_eq!(grid[11][6].date, None);
    }

    #[test]
    fn test_render_heatmap_empty_state() -> Fallible<()> {
        let db = Database::new(":memory:")?;
        let today = Date::new(NaiveDate::from_ymd_opt(2026, 5, 24).unwrap());
        let markup = render_heatmap(&db, today)?;
        let html = markup.into_string();
        assert!(html.contains("No data."));
        assert!(!html.contains("<table"));
        Ok(())
    }

    #[test]
    fn test_build_grid_count_lookup() {
        let today = Date::new(NaiveDate::from_ymd_opt(2026, 5, 20).unwrap());
        let mut counts = HashMap::new();
        counts.insert(date("2026-05-17"), 3);
        counts.insert(date("2026-05-20"), 42);
        let grid = build_grid(today, &counts);
        assert_eq!(grid[11][0].count, 3);
        assert_eq!(grid[11][3].count, 42);
        assert_eq!(grid[11][1].count, 0);
    }
}
