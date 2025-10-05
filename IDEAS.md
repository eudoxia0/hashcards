# Card Stages

Like in Anki. New, learning, mature, with a state machine.

Arguably, the first time a card is reviewed, "forgetting" should not adjust the
FSRS parameters.

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

# Preview Command

Right now the only way to see how a card renders is to run the `drill` command
and hope you see it first. Instead, there should be a `preview` command that
opens a web interface that lets you navigate the flashcards, either all of them,
or one deck at a time, and see how they render.
