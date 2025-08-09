# hashcards

[![Test](https://github.com/eudoxia0/hashcards/actions/workflows/test.yaml/badge.svg)](https://github.com/eudoxia0/hashcards/actions/workflows/test.yaml)
[![codecov](https://codecov.io/gh/eudoxia0/hashcards/branch/master/graph/badge.svg?token=GDV3CYZMHQ)](https://codecov.io/gh/eudoxia0/hashcards)

A plain text-based spaced repetition system.

## Example

The following Markdown file is a valid hashcards deck:

```
What is the capital of France? / Paris

[Paris] is the capital of [France].
```

## Format

A deck is a Markdown file. Blank lines separate flashcards. Question-answer cards use the slash character to separate the sides:

```
What is the order of a group? / The cardinality of its underlying set.
```

Cloze cards use square brackets to denote cloze deletions:

```
The [order] of a group is [the cardinality of its underlying set].
```
