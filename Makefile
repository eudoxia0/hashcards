PREFIX ?= /usr/local
BINDIR = $(PREFIX)/bin
SRC := $(shell find src -name '*.rs')

.PHONY: all
all: hashcards

hashcards: $(SRC) Cargo.toml Cargo.lock
	cargo build --release --target-dir __build
	cp __build/release/hashcards hashcards
	rm -rf __build

.PHONY: install
install: hashcards
	install -d $(BINDIR)
	install -m 755 hashcards $(BINDIR)/hashcards

.PHONY: uninstall
uninstall:
	rm -f $(BINDIR)/hashcards

.PHONY: example
example:
	rm -f example/db.sqlite3
	RUST_LOG=debug cargo run -- drill example

.PHONY: coverage
coverage:
	cargo llvm-cov --html --open --ignore-filename-regex '(main|error|cli).rs'

.PHONY: clean
clean:
	rm -f hashcards
	rm -rf __build
