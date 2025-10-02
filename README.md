# hashcards

[![Test](https://github.com/eudoxia0/hashcards/actions/workflows/test.yaml/badge.svg)](https://github.com/eudoxia0/hashcards/actions/workflows/test.yaml)
[![codecov](https://codecov.io/gh/eudoxia0/hashcards/branch/master/graph/badge.svg?token=GDV3CYZMHQ)](https://codecov.io/gh/eudoxia0/hashcards)

![Screenshot of the app, showing a front/back flashcard.](screenshot.webp)

A plain text-based spaced repetition system. Features:

- **Plain Text:** all your flashcards are stored as plain text files, so you can operate on them with standard tools, write with your editor of choice, and track changes in a VCS.
- **Content Addressable:** cards are identified by the hash of their text. This means a card's progress is reset when the card is edited.
- **Low Friction:** you create flashcards by typing into a text file, using a lightweight notation to denote flashcard sides and cloze deletions.
- **Simple:** the only card types are front-back and cloze cards. More complex workflows (e.g.: Anki-style note types, card templates, automation) be can implemented using a Makefile and some scripts.

## Example

The following Markdown file is a valid hashcards deck:

```
Q: What is the capital of France?
A: Paris

C: [Paris] is the capital of [France].
```

## Building

You need [cargo] installed. You can get it through [rustup]. Then:

```
$ git clone https://github.com/eudoxia0/hashcards.git
$ cd hashcards
$ make
$ sudo make install
```

To drill flashcards in a directory, run:

```
$ hashcards drill $DIRNAME
```

## Format

This section describes the text format used by hashcards.

### Basic Cards

Question-answer flashcards are written like this:

```
Q: What is the order of a group?
A: The cardinality of its underlying set.
```

Both the question and the answer can span multiple lines:

```
Q: List the PGM minerals.
A:

- ruthenium
- rhodium
- palladium
- osmium
- iridium
- platinum
```

### Cloze Cards

Cloze cards start with the `C:` tag, and use square brackets to denote cloze deletions:

```
C: The [order] of a group is [the cardinality of its underlying set].
```

Again, cloze cards can span multiple lines:

```
C:
Better is the sight of the eyes than the wandering of the
desire: this is also vanity and vexation of spirit.

— [Ecclesiastes] [6]:[9]
```

## Database

hashcards stores card performance data and the review history in an SQLite3 database. The file is called `db.sqlite3` and is found in the root of the card directory (i.e., the path you pass to the `drill` command).

The `cards` table has the following schema:

| Column        | Type               | Description                                                                                                         |
|---------------|--------------------|---------------------------------------------------------------------------------------------------------------------|
| `card_hash`   | `text primary key` | The hash of the card.                                                                                               |
| `card_type`   | `text not null`    | One of `basic` or `cloze`.                                                                                          |
| `deck_name`   | `text not null`    | The name of the file where the card was read.                                                                       |
| `question`    | `text not null`    | For a `basic` card, the question text. For a `cloze` card, the prompt text.                                         |
| `answer`      | `text not null`    | For a `basic` card, the answer text. For a `cloze` card, the empty string.                                          |
| `cloze_start` | `integer not null` | For a `cloze` card, the byte position where the cloze deletion starts. For a `basic` card, the number 0.           |
| `cloze_end`   | `integer not null` | For a `cloze` card, the byte position where the cloze deletion ends (inclusive). For a `basic` card, the number 0. |
| `added_at`    | `text not null`    | The timestamp when the card was first added to the database, in [RFC 3339] format.                                 |

The `sessions` table has the following schema:

| Column       | Type                  | Description                                                   |
|--------------|-----------------------|---------------------------------------------------------------|
| `session_id` | `integer primary key` | The ID of the session.                                        |
| `started_at` | `text not null`       | The timestamp when the session started, in [RFC 3339] format. |
| `ended_at`   | `text not null`       | The timestamp when the session ended, in [RFC 3339] format.   |

The `reviews` table has the following schema:

| Column        | Type                  | Description                                                                          |
|---------------|-----------------------|--------------------------------------------------------------------------------------|
| `review_id`   | `integer primary key` | The review ID.                                                                       |
| `session_id`  | `integer not null`    | The ID of the session this review was performed in, a foreign key.                   |
| `card_hash`   | `text not null`       | The hash of the card that was reviewed, a foreign key.                               |
| `reviewed_at` | `text not null`       | The timestamp when the review was performed (i.e., when the user submitted a grade). |
| `grade`       | `text not null`       | One of `forgot`, `hard`, `good`, `easy`.                                             |
| `stability`   | `real not null`       | The card's stability after this review.                                              |
| `difficulty`  | `real not null`       | The card's difficulty after this review.                                             |
| `due_date`    | `text not null`       | The date, in the user's local time, when the card is next due.                       |

[RFC 3339]: https://datatracker.ietf.org/doc/html/rfc3339

## Prior Art

- [org-fc](https://github.com/l3kn/org-fc)
- [org-drill](https://orgmode.org/worg/org-contrib/org-drill.html)
- [hascard](https://hackage.haskell.org/package/hascard)
- [carddown](https://github.com/martintrojer/carddown)

[cargo]: https://doc.rust-lang.org/cargo/
[rustup]: https://rustup.rs/
