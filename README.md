# hashcards

A plain text-based spaced repetition system.

## Example

The following Markdown file is a valid hashcards deck:

```
Term for the difference in electric potential between two points. / Voltage

A voltage exists whenever [positive and negative charges are separated].
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
