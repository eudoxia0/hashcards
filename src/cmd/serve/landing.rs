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
    let git_enabled = state.config.git.is_some();
    let html = render_landing_page(&collections, last_synced, git_enabled);
    (StatusCode::OK, Html(html.into_string()))
}

fn render_landing_page(
    collections: &[CollectionInfo],
    last_synced: Option<Timestamp>,
    git_enabled: bool,
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
