PREFIX ?= /usr/local
BINDIR = $(PREFIX)/bin
SRC    = $(shell find src -name '*.rs')
KATEX_VERSION = 0.16.25
KATEX_URL = https://github.com/KaTeX/KaTeX/releases/download/v$(KATEX_VERSION)/katex.tar.gz

.PHONY: all
all: hashcards

vendor/katex:
	@echo "Downloading KaTeX $(KATEX_VERSION)..."
	@mkdir -p vendor
	@curl -L -o vendor/katex.tar.gz $(KATEX_URL)
	@echo "Extracting KaTeX..."
	@tar -xzf vendor/katex.tar.gz -C vendor
	@rm vendor/katex.tar.gz
	@echo "Rewriting font paths in CSS..."
	@sed -i.bak 's|fonts/|/katex/fonts/|g' vendor/katex/katex.min.css
	@rm vendor/katex/katex.min.css.bak
	@echo "KaTeX extracted to vendor/katex"

hashcards: vendor/katex $(SRC) Cargo.toml Cargo.lock
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

.PHONY: install-hooks
install-hooks: pre-commit.sh
	@mkdir -p .git/hooks
	@ln -sf ../../pre-commit.sh .git/hooks/pre-commit

.PHONY: uninstall-hooks
uninstall-hooks:
	@rm -f .git/hooks/pre-commit

.PHONY: clean
clean:
	rm -f hashcards
	rm -rf vendor
	cargo clean
