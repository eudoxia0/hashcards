use maud::Markup;
use maud::html;

use crate::cmd::drill::template::page_template;
use crate::cmd::serve::state::HedgedocSource;
use crate::types::timestamp::Timestamp;

pub fn render_manage_page(
    sources: &[HedgedocSource],
    last_synced: Option<Timestamp>,
    config_available: bool,
) -> Markup {
    page_template(html! {
        div.landing {
            h1 { "HedgeDoc Sources" }
            p { a href="/" { "← Back to collections" } }

            @if !config_available {
                div.notice {
                    p { "Note: HedgeDoc source changes will be kept in memory for this session. " }
                    p { "To persist them permanently, start hashcards with " code { "--config hashcards.toml" } "." }
                }
            } @else {
                div.sync-bar {
                    span.sync-status {
                        @if let Some(ts) = last_synced {
                            (format!("Last synced: {}", ts.into_inner().format("%Y-%m-%d %H:%M:%S")))
                        } @else {
                            "Not yet synced"
                        }
                    }
                    form.inline-form action="/hedgedoc/sync" method="post" {
                        input .sync-button type="submit" value="Sync All";
                    }
                }

                h2 { "Add Source" }
                form.add-source-form action="/hedgedoc/add" method="post" {
                    div.add-source-row {
                        input
                            .add-source-url
                            type="url"
                            name="url"
                            placeholder="https://notes.example.com/noteId"
                            required;
                        input type="submit" value="Add" .sync-button;
                    }
                }

                @if sources.is_empty() {
                    p.empty { "No HedgeDoc sources configured." }
                } @else {
                    h2 { "Sources" }
                    table.collection-table {
                        thead {
                            tr {
                                th { "Source" }
                                th { "Deck" }
                                th { "URL" }
                                th { "Status" }
                                th { "" }
                            }
                        }
                        tbody {
                            @for src in sources {
                                @for note in &src.notes {
                                    tr {
                                        td { (src.collection.name) }
                                        td { (note.deck_name) }
                                        td.source-url-cell {
                                            a href=(note.url) target="_blank" rel="noopener noreferrer" { (note.url) }
                                        }
                                        td {
                                            @if let Some(ref err) = note.last_error {
                                                span.status-error title=(err) { "Error" }
                                            } @else {
                                                span.status-ok { "OK" }
                                            }
                                        }
                                        td {
                                            form action="/hedgedoc/delete" method="post" {
                                                input type="hidden" name="url" value=(note.url);
                                                input type="submit" value="Delete" .sync-button
                                                    onclick="return confirm('Remove this HedgeDoc source?')";
                                            }
                                        }
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
