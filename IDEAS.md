# Card Stages

Like in Anki. New, learning, mature, with a state machine.

Arguably, the first time a card is reviewed, "forgetting" should not adjust the FSRS parameters.

See:

- <https://docs.ankiweb.net/getting-started.html#card-states>

# Term-Definition Cards

A shorthand. Writing:

```
T: lithification
D: The process of turning loose sediment into rock.
```

Is equivalent to writing this:

```
Q: Define: lithification
A: The process of turning loose sediment into rock.

Q: Term for: The process of turning loose sediment into rock.
A: lithification
```

# Burying Siblings

Two cloze cards are siblings if they are derived from the same source text.

In Anki, siblings are "buried" (i.e.: not shown) within a session. The reason is obvious: seeing one card spoils the answer for another.

# Strongy-Typed Parser Errors

The parser state machine makes this easy.

# Deck Datatype

Instead of `Vec<Card>`, use a `Deck` object where appropriate.
