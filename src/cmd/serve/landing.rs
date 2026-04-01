use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use maud::Markup;
use maud::html;

use crate::cmd::drill::template::page_template;
use crate::cmd::serve::state::AppState;
use crate::cmd::serve::state::CollectionInfo;
use crate::types::timestamp::Timestamp;

pub async fn landing_handler(State(state): State<AppState>) -> (StatusCode, Html<String>) {
    let collections = state.collections.read().await;
    let last_synced = *state.last_synced.lock().unwrap();
    let hedgedoc_last_synced = *state.hedgedoc_last_synced.lock().unwrap();
    let git_enabled = state.config.git.is_some();
    let hedgedoc_count = state.hedgedoc_sources.lock().unwrap().len();
    let config_available = state.config.data_dir.is_some();
    let html = render_landing_page(
        &collections,
        last_synced,
        git_enabled,
        hedgedoc_count,
        hedgedoc_last_synced,
        config_available,
    );
    (StatusCode::OK, Html(html.into_string()))
}

fn render_landing_page(
    collections: &[CollectionInfo],
    last_synced: Option<Timestamp>,
    git_enabled: bool,
    hedgedoc_count: usize,
    hedgedoc_last_synced: Option<Timestamp>,
    config_available: bool,
) -> Markup {
    page_template(html! {
        div.landing {
            h1 { "hashcards" }
            @if git_enabled {
                div.sync-bar {
                    span.sync-status {
                        @if let Some(ts) = last_synced {
                            (format!("Last synced: {}", ts.into_inner().format("%Y-%m-%d %H:%M:%S")))
                        } @else {
                            "Not yet synced"
                        }
                    }
                    form action="/sync" method="post" style="display:inline" {
                        input .sync-button type="submit" value="Sync Now";
                    }
                }
            }
            @if config_available {
                div.sync-bar {
                    span.sync-status {
                        @if hedgedoc_count > 0 {
                            @if let Some(ts) = hedgedoc_last_synced {
                                (format!("HedgeDoc synced: {}", ts.into_inner().format("%Y-%m-%d %H:%M:%S")))
                            } @else {
                                (format!("{hedgedoc_count} HedgeDoc source(s) — not yet synced"))
                            }
                        } @else {
                            "No HedgeDoc sources"
                        }
                    }
                    form action="/hedgedoc" method="get" style="display:inline" {
                        input .sync-button type="submit" value="Manage HedgeDoc";
                    }
                }
            }
            @if collections.is_empty() {
                p.empty { "No collections configured." }
            } @else {
                table.collection-table {
                    thead {
                        tr {
                            th { "Collection" }
                            th { "Due Today" }
                            th { "Total Cards" }
                            th { "" }
                        }
                    }
                    tbody {
                        @for coll in collections {
                            tr class=@if coll.due_today == 0 { "muted" } {
                                td { (coll.name.clone()) }
                                td.num { (coll.due_today) }
                                td.num { (coll.total_cards) }
                                td {
                                    @if coll.due_today > 0 {
                                        a.drill-link href=(format!("/collection/{}", coll.slug)) {
                                            "Drill"
                                        }
                                    } @else {
                                        span.no-cards { "Nothing due" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}
