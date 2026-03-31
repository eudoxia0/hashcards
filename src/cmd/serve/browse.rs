use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use maud::Markup;
use maud::html;

use crate::cmd::drill::template::page_template;
use crate::collection::Collection;
use crate::error::Fallible;
use crate::types::card_hash::CardHash;
use crate::types::date::Date;
use crate::types::timestamp::Timestamp;

/// A node in the deck tree. Leaves have cards; parents aggregate children.
pub struct DeckNode {
    /// Display name for this segment (e.g., "particles").
    pub name: String,
    /// Full deck path (e.g., "grammar/particles"). Empty for the root.
    pub path: String,
    /// Number of cards directly in this deck (0 for pure parent nodes).
    pub total_cards: usize,
    /// Number of cards due today directly in this deck.
    pub due_today: usize,
    /// Child nodes.
    pub children: Vec<DeckNode>,
}

impl DeckNode {
    /// Total cards in this node and all descendants.
    pub fn total_cards_recursive(&self) -> usize {
        self.total_cards + self.children.iter().map(|c| c.total_cards_recursive()).sum::<usize>()
    }

    /// Due cards in this node and all descendants.
    pub fn due_today_recursive(&self) -> usize {
        self.due_today + self.children.iter().map(|c| c.due_today_recursive()).sum::<usize>()
    }
}

/// Counts per deck name.
struct DeckCounts {
    total: usize,
    due: usize,
}

/// Build a deck tree from a collection, computing per-deck due/total counts.
pub fn build_deck_tree(coll_dir: &Path, db_path: &Path) -> Fallible<DeckNode> {
    let collection = Collection::with_db_path(coll_dir.to_path_buf(), db_path.to_path_buf())?;
    let session_started_at = Timestamp::now();
    let today: Date = session_started_at.date();

    // Sync new cards to DB
    let db_hashes: HashSet<CardHash> = collection.db.card_hashes()?;
    for card in collection.cards.iter() {
        if !db_hashes.contains(&card.hash()) {
            collection.db.insert_card(card.hash(), session_started_at)?;
        }
    }

    let due_hashes: HashSet<CardHash> = collection.db.due_today(today)?;

    // Count per deck
    let mut counts: HashMap<String, DeckCounts> = HashMap::new();
    for card in &collection.cards {
        let entry = counts
            .entry(card.deck_name().clone())
            .or_insert(DeckCounts { total: 0, due: 0 });
        entry.total += 1;
        if due_hashes.contains(&card.hash()) {
            entry.due += 1;
        }
    }

    Ok(build_tree_from_counts(counts))
}

/// Build a hierarchical tree from flat deck name → counts mapping.
fn build_tree_from_counts(counts: HashMap<String, DeckCounts>) -> DeckNode {
    let mut root = DeckNode {
        name: String::new(),
        path: String::new(),
        total_cards: 0,
        due_today: 0,
        children: Vec::new(),
    };

    // Sort deck names for deterministic ordering.
    let mut names: Vec<String> = counts.keys().cloned().collect();
    names.sort();

    for deck_name in names {
        let deck_counts = &counts[&deck_name];
        let segments: Vec<&str> = deck_name.split('/').collect();
        insert_into_tree(&mut root, &segments, 0, &deck_name, deck_counts);
    }

    root
}

fn insert_into_tree(
    node: &mut DeckNode,
    segments: &[&str],
    depth: usize,
    full_path: &str,
    counts: &DeckCounts,
) {
    if depth == segments.len() {
        // We've reached the leaf — set counts on this node.
        node.total_cards = counts.total;
        node.due_today = counts.due;
        return;
    }

    let segment = segments[depth];

    // Find or create child for this segment.
    let child_idx = node.children.iter().position(|c| c.name == segment);
    let child_idx = match child_idx {
        Some(idx) => idx,
        None => {
            let child_path = if depth + 1 == segments.len() {
                full_path.to_string()
            } else {
                segments[..=depth].join("/")
            };
            node.children.push(DeckNode {
                name: segment.to_string(),
                path: child_path,
                total_cards: 0,
                due_today: 0,
                children: Vec::new(),
            });
            node.children.len() - 1
        }
    };

    insert_into_tree(&mut node.children[child_idx], segments, depth + 1, full_path, counts);
}

/// Render the deck browser page for a collection.
pub fn render_browse_page(collection_name: &str, slug: &str, tree: &DeckNode) -> Markup {
    let total_due = tree.due_today_recursive();
    page_template(html! {
        div.browse {
            div.browse-header {
                a.back-link href="/" { "\u{2190} Collections" }
                h1 { (collection_name) }
            }
            @if tree.children.is_empty() {
                p.empty { "No decks found in this collection." }
            } @else {
                form action=(format!("/collection/{slug}/start")) method="post" {
                    div.deck-tree {
                        @for child in &tree.children {
                            (render_deck_node(child, 0))
                        }
                    }
                    div.browse-controls {
                        span.select-controls {
                            a.select-all href="#" onclick="selectAll(true); return false;" { "Select all" }
                            " / "
                            a.select-none href="#" onclick="selectAll(false); return false;" { "Select none" }
                        }
                        input
                            type="submit"
                            value=(format!("Drill ({total_due} due)"))
                            class="drill-button"
                            disabled[total_due == 0];
                    }
                }
                script {
                    (maud::PreEscaped(BROWSE_SCRIPT))
                }
            }
        }
    })
}

fn render_deck_node(node: &DeckNode, depth: usize) -> Markup {
    let total = node.total_cards_recursive();
    let due = node.due_today_recursive();
    let has_children = !node.children.is_empty();

    html! {
        div.deck-node {
            div.deck-row style=(format!("padding-left: {}px", depth * 24)) {
                @if has_children {
                    span.toggle-children onclick="toggleChildren(this)" { "\u{25bc}" }
                } @else {
                    span.toggle-placeholder {}
                }
                label.deck-label {
                    @if has_children {
                        input
                            type="checkbox"
                            checked[due > 0]
                            data-parent
                            onchange="onCheckboxChange(this)";
                    } @else {
                        input
                            type="checkbox"
                            name="decks"
                            value=(node.path)
                            checked[due > 0]
                            onchange="onCheckboxChange(this)";
                    }
                    span.deck-name { (node.name) }
                }
                span.deck-counts {
                    span.deck-due class=@if due == 0 { "muted" } { (due) }
                    " / "
                    span.deck-total { (total) }
                }
            }
            @if has_children {
                div.deck-children {
                    @for child in &node.children {
                        (render_deck_node(child, depth + 1))
                    }
                }
            }
        }
    }
}

const BROWSE_SCRIPT: &str = r#"
function selectAll(checked) {
    document.querySelectorAll('.deck-tree input[type="checkbox"]').forEach(function(cb) {
        cb.checked = checked;
    });
    updateDrillButton();
}

function toggleChildren(el) {
    var children = el.closest('.deck-node').querySelector('.deck-children');
    if (children) {
        var collapsed = children.style.display === 'none';
        children.style.display = collapsed ? '' : 'none';
        el.textContent = collapsed ? '\u25bc' : '\u25b6';
    }
}

function onCheckboxChange(cb) {
    // If this is a parent checkbox, toggle all children
    if (cb.hasAttribute('data-parent')) {
        var children = cb.closest('.deck-node').querySelector('.deck-children');
        if (children) {
            children.querySelectorAll('input[type="checkbox"]').forEach(function(child) {
                child.checked = cb.checked;
            });
        }
    }
    // Update parent checkboxes
    updateParentCheckboxes(cb);
    updateDrillButton();
}

function updateParentCheckboxes(cb) {
    var parentNode = cb.closest('.deck-node').parentElement;
    if (!parentNode || !parentNode.classList.contains('deck-children')) return;
    var grandparent = parentNode.closest('.deck-node');
    if (!grandparent) return;
    var parentCb = grandparent.querySelector(':scope > .deck-row input[type="checkbox"]');
    if (!parentCb) return;
    var siblings = parentNode.querySelectorAll(':scope > .deck-node > .deck-row input[type="checkbox"]');
    var allChecked = true;
    var anyChecked = false;
    siblings.forEach(function(s) {
        if (s.checked) anyChecked = true;
        else allChecked = false;
    });
    parentCb.checked = anyChecked;
    parentCb.indeterminate = anyChecked && !allChecked;
    updateParentCheckboxes(parentCb);
}

function updateDrillButton() {
    var btn = document.querySelector('.drill-button');
    if (!btn) return;
    // Count due cards from checked leaf checkboxes
    var checked = document.querySelectorAll('.deck-tree input[type="checkbox"]:checked:not([data-parent])');
    var totalDue = 0;
    checked.forEach(function(cb) {
        var row = cb.closest('.deck-row');
        var dueEl = row.querySelector('.deck-due');
        if (dueEl) totalDue += parseInt(dueEl.textContent) || 0;
    });
    btn.value = 'Drill (' + totalDue + ' due)';
    btn.disabled = totalDue === 0;
}
"#;
