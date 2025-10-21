PREFIX ?= /usr/local
BINDIR = $(PREFIX)/bin
SRC    = $(shell find src -name '*.rs')

.PHONY: all
all: hashcards

hashcards: $(SRC) Cargo.toml Cargo.lock
	cargo build --release
	cp "target/release/hashcards" hashcards

.PHONY: install
install: hashcards
	install -d $(BINDIR)
	install -m 755 hashcards $(BINDIR)/hashcards

.PHONY: uninstall
uninstall:
	rm -f $(BINDIR)/hashcards

.PHONY: example
example:
	rm -f example/hashcards.db
	RUST_LOG=debug cargo run -- drill example

.PHONY: coverage
coverage:
	cargo llvm-cov --html --open --ignore-filename-regex '(main|error|cli).rs'

.PHONY: clean
clean:
	rm -f hashcards
	cargo clean
