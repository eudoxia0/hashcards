pragma foreign_keys = on;

create table cards (
    card_hash text primary key,
    card_type text not null,
    deck_name text not null,
    question text not null,
    answer text not null,
    cloze_start integer not null,
    cloze_end integer not null
) strict;

create table sessions (
    session_id integer primary key,
    started_at text not null,
    ended_at text not null
) strict;

create table reviews (
    review_id integer primary key,
    session_id integer not null
        references sessions (session_id)
        on update cascade
        on delete cascade,
    card_hash text not null
        references cards (card_hash)
        on update cascade
        on delete cascade,
    reviewed_at text not null,
    grade text not null,
    stability real not null,
    difficulty real not null,
    next_due_date text not null
) strict;
