default: hashcards

hashcards: src/**.rs Cargo.toml Cargo.lock
	cargo build --release --target-dir __build
	cp __build/release/hashcards hashcards
	rm -rf __build

.PHONY: clean
clean:
	rm -f hashcards
	rm -rf __build
