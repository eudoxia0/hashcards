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
                div.error {
                    p { "HedgeDoc source management requires a config file. Start hashcards with " code { "--config hashcards.toml" } "." }
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
                    form action="/hedgedoc/sync" method="post" style="display:inline" {
                        input .sync-button type="submit" value="Sync All";
                    }
                }

                h2 { "Add Source" }
                form action="/hedgedoc/add" method="post" {
                    div style="display:flex;gap:0.5rem;align-items:center" {
                        input
                            type="url"
                            name="url"
                            placeholder="https://notes.example.com/noteId"
                            required
                            style="flex:1;padding:0.4rem 0.6rem;font-size:1rem";
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
                                th { "Name" }
                                th { "URL" }
                                th { "Status" }
                                th { "" }
                            }
                        }
                        tbody {
                            @for src in sources {
                                tr {
                                    td { (src.collection.name) }
                                    td style="font-size:0.85rem;word-break:break-all" {
                                        a href=(src.url) target="_blank" { (src.url) }
                                    }
                                    td {
                                        @if let Some(ref err) = src.last_error {
                                            span style="color:var(--error-color,#c00)" title=(err) { "Error" }
                                        } @else {
                                            span style="color:var(--success-color,#080)" { "OK" }
                                        }
                                    }
                                    td {
                                        form action="/hedgedoc/delete" method="post" {
                                            input type="hidden" name="url" value=(src.url);
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
    })
}
